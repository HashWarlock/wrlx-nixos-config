//! File watcher for dynamic skill reloading.
//!
//! Monitors the skills directory for changes and triggers reloads when
//! instruction or workflow files are modified.

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use super::loader::SkillsLoader;

/// Event indicating skills have been modified.
#[derive(Debug, Clone)]
pub enum SkillEvent {
    /// One or more skills were modified and reloaded.
    Reloaded { count: usize },
    /// An error occurred during reload.
    Error { message: String },
}

/// Watches the skills directory and reloads skills on changes.
pub struct SkillsWatcher {
    skills_dir: PathBuf,
    loader: Arc<RwLock<SkillsLoader>>,
    #[allow(dead_code)]
    debouncer: Option<Debouncer<RecommendedWatcher>>,
}

impl SkillsWatcher {
    /// Creates a new skills watcher.
    ///
    /// The watcher will monitor the skills directory and reload skills
    /// when files are added, modified, or removed.
    pub fn new(skills_dir: PathBuf, loader: Arc<RwLock<SkillsLoader>>) -> Self {
        Self {
            skills_dir,
            loader,
            debouncer: None,
        }
    }

    /// Starts watching for changes.
    ///
    /// Returns a channel that receives events when skills are reloaded.
    pub fn start(&mut self) -> Result<mpsc::Receiver<SkillEvent>, anyhow::Error> {
        let (tx, rx) = mpsc::channel(16);
        let loader = self.loader.clone();
        let _skills_dir = self.skills_dir.clone();

        // Get the runtime handle to spawn tasks from the callback thread
        let handle = tokio::runtime::Handle::current();

        // Create debounced watcher
        let debouncer = new_debouncer(
            Duration::from_millis(500),
            move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                let tx = tx.clone();
                let loader = loader.clone();
                let handle = handle.clone();

                // Spawn async task to handle the event using the captured runtime handle
                handle.spawn(async move {
                    match result {
                        Ok(events) => {
                            // Check if any skill files were affected
                            let skill_modified = events.iter().any(|e| {
                                matches!(e.kind, DebouncedEventKind::Any | DebouncedEventKind::AnyContinuous)
                                    && is_skill_file(&e.path)
                            });

                            if skill_modified {
                                // Reload all skills
                                let mut loader = loader.write().await;
                                match loader.load_all().await {
                                    Ok(()) => {
                                        let count = loader.list().len();
                                        tracing::info!("Hot-reloaded {} skills", count);
                                        let _ = tx.send(SkillEvent::Reloaded { count }).await;
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to reload skills: {}", e);
                                        let _ = tx.send(SkillEvent::Error {
                                            message: e.to_string(),
                                        }).await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Watch error: {}", e);
                            let _ = tx.send(SkillEvent::Error {
                                message: e.to_string(),
                            }).await;
                        }
                    }
                });
            },
        )?;

        // Start watching the skills directory
        self.debouncer = Some(debouncer);
        if let Some(ref mut debouncer) = self.debouncer {
            debouncer
                .watcher()
                .watch(&self.skills_dir, RecursiveMode::Recursive)?;
            tracing::info!("Watching skills directory: {}", self.skills_dir.display());
        }

        Ok(rx)
    }

    /// Stops watching for changes.
    #[allow(dead_code)]
    pub fn stop(&mut self) {
        if let Some(mut debouncer) = self.debouncer.take() {
            let _ = debouncer.watcher().unwatch(&self.skills_dir);
            tracing::info!("Stopped watching skills directory");
        }
    }
}

/// Checks if a path is a skill file (markdown or yaml).
fn is_skill_file(path: &std::path::Path) -> bool {
    path.extension()
        .map(|ext| ext == "md" || ext == "yaml" || ext == "yml")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::loader::SkillsLoader;
    use tempfile::TempDir;
    use tokio::time::{timeout, Duration};

    #[test]
    fn test_is_skill_file() {
        assert!(is_skill_file(std::path::Path::new("skill.md")));
        assert!(is_skill_file(std::path::Path::new("workflow.yaml")));
        assert!(is_skill_file(std::path::Path::new("flow.yml")));
        assert!(!is_skill_file(std::path::Path::new("readme.txt")));
        assert!(!is_skill_file(std::path::Path::new("data.json")));
    }

    /// Helper to create a valid skill file content
    fn valid_skill_content(name: &str) -> String {
        format!(
            r#"---
name: {}
description: A test skill
triggers:
  - test trigger
---

# Test Skill

This is the body content.
"#,
            name
        )
    }

    /// Test 1: Create a skill file and verify reload event is received
    #[tokio::test]
    async fn test_watch_and_reload() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().to_path_buf();

        // Create instructions subdirectory
        let instructions_dir = skills_dir.join("instructions");
        std::fs::create_dir_all(&instructions_dir).unwrap();

        // Create loader and watcher
        let loader = Arc::new(RwLock::new(SkillsLoader::new(skills_dir.to_str().unwrap())));
        let mut watcher = SkillsWatcher::new(skills_dir.clone(), loader.clone());

        // Start watching
        let mut rx = watcher.start().expect("Failed to start watcher");

        // Give the watcher time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Write a new skill file
        let skill_path = instructions_dir.join("new-skill.md");
        std::fs::write(&skill_path, valid_skill_content("new-skill")).unwrap();

        // Wait for the reload event with a generous timeout (debounce is 500ms + processing time)
        let result = timeout(Duration::from_secs(3), rx.recv()).await;

        assert!(result.is_ok(), "Should receive event within timeout");
        let event = result.unwrap();
        assert!(event.is_some(), "Channel should not be closed");

        match event.unwrap() {
            SkillEvent::Reloaded { count } => {
                assert!(count >= 1, "Should have at least 1 skill loaded, got {}", count);
            }
            SkillEvent::Error { message } => {
                panic!("Unexpected error event: {}", message);
            }
        }

        // Verify the loader has the skill
        let loader_guard = loader.read().await;
        assert!(loader_guard.get("new-skill").is_some(), "Loader should have the new skill");
    }

    /// Test 2: Rapid file changes should be debounced into fewer events
    #[tokio::test]
    async fn test_debounce_multiple_changes() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().to_path_buf();

        // Create instructions subdirectory
        let instructions_dir = skills_dir.join("instructions");
        std::fs::create_dir_all(&instructions_dir).unwrap();

        // Create an initial skill file
        let skill_path = instructions_dir.join("debounce-test.md");
        std::fs::write(&skill_path, valid_skill_content("debounce-test")).unwrap();

        // Create loader and watcher
        let loader = Arc::new(RwLock::new(SkillsLoader::new(skills_dir.to_str().unwrap())));
        let mut watcher = SkillsWatcher::new(skills_dir.clone(), loader.clone());

        // Start watching
        let mut rx = watcher.start().expect("Failed to start watcher");

        // Give the watcher time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Make multiple rapid changes (all within the 500ms debounce window)
        for i in 0..5 {
            std::fs::write(&skill_path, valid_skill_content(&format!("debounce-test-v{}", i))).unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await; // 50ms between changes
        }

        // Count how many events we receive
        // Wait long enough for debounce to complete (500ms) plus processing time
        let mut event_count = 0;
        let collection_window = Duration::from_secs(2);
        let start = std::time::Instant::now();

        while start.elapsed() < collection_window {
            match timeout(Duration::from_millis(200), rx.recv()).await {
                Ok(Some(SkillEvent::Reloaded { .. })) => {
                    event_count += 1;
                }
                Ok(Some(SkillEvent::Error { message })) => {
                    panic!("Unexpected error: {}", message);
                }
                Ok(None) => break, // Channel closed
                Err(_) => continue, // Timeout, keep waiting
            }
        }

        // With debouncing, we should get significantly fewer events than the 5 changes we made
        // The debouncer batches events within the 500ms window, so we expect 1-2 events max
        assert!(
            event_count >= 1 && event_count <= 2,
            "Expected 1-2 debounced events, got {}",
            event_count
        );
    }

    /// Test 3: Stopping the watcher should prevent further events
    #[tokio::test]
    async fn test_stop_watching() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path().to_path_buf();

        // Create instructions subdirectory
        let instructions_dir = skills_dir.join("instructions");
        std::fs::create_dir_all(&instructions_dir).unwrap();

        // Create loader and watcher
        let loader = Arc::new(RwLock::new(SkillsLoader::new(skills_dir.to_str().unwrap())));
        let mut watcher = SkillsWatcher::new(skills_dir.clone(), loader.clone());

        // Start watching
        let mut rx = watcher.start().expect("Failed to start watcher");

        // Give the watcher time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Stop the watcher
        watcher.stop();

        // Give the stop time to take effect
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Make file changes after stopping
        let skill_path = instructions_dir.join("after-stop.md");
        std::fs::write(&skill_path, valid_skill_content("after-stop")).unwrap();

        // Wait for potential events (longer than debounce window)
        tokio::time::sleep(Duration::from_millis(700)).await;

        // Try to receive - should timeout with no events
        let result = timeout(Duration::from_millis(500), rx.recv()).await;

        // Either timeout (no event) or channel closed (None) - both are acceptable
        match result {
            Err(_) => {
                // Timeout - no event received, which is correct
            }
            Ok(None) => {
                // Channel closed - also acceptable
            }
            Ok(Some(event)) => {
                panic!("Should not receive events after stop, got: {:?}", event);
            }
        }
    }
}
