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
        let skills_dir = self.skills_dir.clone();

        // Create debounced watcher
        let debouncer = new_debouncer(
            Duration::from_millis(500),
            move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                let tx = tx.clone();
                let loader = loader.clone();
                let skills_dir = skills_dir.clone();

                // Spawn async task to handle the event
                tokio::spawn(async move {
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

    #[test]
    fn test_is_skill_file() {
        assert!(is_skill_file(std::path::Path::new("skill.md")));
        assert!(is_skill_file(std::path::Path::new("workflow.yaml")));
        assert!(is_skill_file(std::path::Path::new("flow.yml")));
        assert!(!is_skill_file(std::path::Path::new("readme.txt")));
        assert!(!is_skill_file(std::path::Path::new("data.json")));
    }
}
