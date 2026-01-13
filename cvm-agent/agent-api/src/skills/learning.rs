//! Self-improving skill system for the CVM Agent.
//!
//! This module handles learning from failures by:
//! 1. Searching existing lessons for similar issues
//! 2. Creating new instruction skills when gaps are found
//! 3. Storing lessons learned for future reference

use std::path::PathBuf;
use tokio::fs;

use crate::db::{DbPool, LessonsRepository};

/// Manages learning from failures and skill creation.
pub struct SkillLearner {
    lessons_repo: LessonsRepository,
    skills_dir: PathBuf,
}

/// Result of searching for a solution to a problem.
#[derive(Debug)]
pub struct LessonMatch {
    pub lesson_id: String,
    pub trigger_pattern: String,
    pub solution: String,
    pub confidence: f64,
}

/// Request to create a new instruction skill.
#[derive(Debug)]
pub struct NewSkillRequest {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub content: String,
}

impl SkillLearner {
    /// Creates a new skill learner.
    pub fn new(db: DbPool, skills_dir: PathBuf) -> Self {
        Self {
            lessons_repo: LessonsRepository::new(db),
            skills_dir,
        }
    }

    /// Searches for a lesson that matches the given failure context.
    ///
    /// Returns the best matching lesson if one exists with sufficient confidence.
    pub fn find_lesson(&self, failure_context: &str) -> Option<LessonMatch> {
        // Search lessons for similar issues
        let results = self
            .lessons_repo
            .search(failure_context, 5)
            .ok()?;

        // Find best match with confidence > 0.5
        results
            .into_iter()
            .filter(|r| r.confidence() > 0.5)
            .max_by(|a, b| a.confidence().partial_cmp(&b.confidence()).unwrap())
            .map(|r| {
                let confidence = r.confidence();
                LessonMatch {
                    lesson_id: r.id,
                    trigger_pattern: r.trigger_pattern,
                    solution: r.solution,
                    confidence,
                }
            })
    }

    /// Stores a new lesson learned from a problem-solution pair.
    pub fn store_lesson(
        &self,
        trigger_pattern: &str,
        solution: &str,
        context: &str,
    ) -> Result<String, anyhow::Error> {
        let id = self
            .lessons_repo
            .store(trigger_pattern, solution, context)?;
        tracing::info!("Stored new lesson: {} -> {}", trigger_pattern, solution);
        Ok(id)
    }

    /// Records that a lesson's solution worked.
    pub fn record_success(&self, lesson_id: &str) -> Result<(), anyhow::Error> {
        self.lessons_repo.record_success(lesson_id)?;
        tracing::debug!("Recorded success for lesson {}", lesson_id);
        Ok(())
    }

    /// Records that a lesson's solution failed.
    pub fn record_failure(&self, lesson_id: &str) -> Result<(), anyhow::Error> {
        self.lessons_repo.record_failure(lesson_id)?;
        tracing::debug!("Recorded failure for lesson {}", lesson_id);
        Ok(())
    }

    /// Creates a new instruction skill from the provided content.
    ///
    /// This is called when the agent discovers a knowledge gap and wants
    /// to create a skill for future use.
    pub async fn create_instruction_skill(
        &self,
        request: NewSkillRequest,
    ) -> Result<PathBuf, anyhow::Error> {
        let instructions_dir = self.skills_dir.join("instructions");
        fs::create_dir_all(&instructions_dir).await?;

        // Generate filename from skill name
        let filename = format!("{}.md", request.name.to_lowercase().replace(' ', "-"));
        let path = instructions_dir.join(&filename);

        // Build skill content with YAML frontmatter
        let triggers_yaml = request
            .triggers
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        let content = format!(
            r#"---
name: {}
description: {}
triggers:
{}
---

{}
"#,
            request.name, request.description, triggers_yaml, request.content
        );

        fs::write(&path, content).await?;
        tracing::info!("Created new instruction skill: {}", path.display());

        Ok(path)
    }

    /// Updates an existing instruction skill with new content.
    pub async fn update_instruction_skill(
        &self,
        skill_name: &str,
        additional_content: &str,
    ) -> Result<(), anyhow::Error> {
        let instructions_dir = self.skills_dir.join("instructions");
        let filename = format!("{}.md", skill_name.to_lowercase().replace(' ', "-"));
        let path = instructions_dir.join(&filename);

        if !path.exists() {
            anyhow::bail!("Skill not found: {}", skill_name);
        }

        // Read existing content
        let existing = fs::read_to_string(&path).await?;

        // Append new content
        let updated = format!("{}\n\n## Additional Notes\n\n{}", existing.trim(), additional_content);

        fs::write(&path, updated).await?;
        tracing::info!("Updated instruction skill: {}", path.display());

        Ok(())
    }

    /// Analyzes a failure and returns a structured analysis.
    pub fn analyze_failure(
        &self,
        action: &str,
        error: &str,
        context: Option<&str>,
    ) -> FailureAnalysis {
        // Extract key information from error
        let error_type = Self::classify_error(error);
        let search_query = Self::build_search_query(action, error, context);

        // Check for existing lessons
        let existing_lesson = self.find_lesson(&search_query);

        FailureAnalysis {
            action: action.to_string(),
            error: error.to_string(),
            error_type,
            search_query,
            existing_lesson,
        }
    }

    /// Classifies an error into categories.
    fn classify_error(error: &str) -> ErrorType {
        let error_lower = error.to_lowercase();

        if error_lower.contains("not found") || error_lower.contains("no such file") {
            ErrorType::NotFound
        } else if error_lower.contains("permission denied") {
            ErrorType::PermissionDenied
        } else if error_lower.contains("timeout") {
            ErrorType::Timeout
        } else if error_lower.contains("syntax") || error_lower.contains("parse") {
            ErrorType::SyntaxError
        } else if error_lower.contains("dependency") || error_lower.contains("requires") {
            ErrorType::DependencyError
        } else if error_lower.contains("nixos") || error_lower.contains("nix") {
            ErrorType::NixError
        } else {
            ErrorType::Unknown
        }
    }

    /// Builds a search query from failure context.
    fn build_search_query(action: &str, error: &str, context: Option<&str>) -> String {
        let mut query = format!("{} {}", action, error);
        if let Some(ctx) = context {
            query.push(' ');
            query.push_str(ctx);
        }
        // Truncate to reasonable length for search
        if query.len() > 200 {
            query.truncate(200);
        }
        query
    }
}

/// Analysis of a failure for learning purposes.
#[derive(Debug)]
pub struct FailureAnalysis {
    pub action: String,
    pub error: String,
    pub error_type: ErrorType,
    pub search_query: String,
    pub existing_lesson: Option<LessonMatch>,
}

/// Categories of errors for targeted solutions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorType {
    NotFound,
    PermissionDenied,
    Timeout,
    SyntaxError,
    DependencyError,
    NixError,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_error_not_found() {
        assert_eq!(
            SkillLearner::classify_error("file not found: /etc/foo"),
            ErrorType::NotFound
        );
    }

    #[test]
    fn test_classify_error_permission() {
        assert_eq!(
            SkillLearner::classify_error("permission denied: /root/.ssh"),
            ErrorType::PermissionDenied
        );
    }

    #[test]
    fn test_classify_error_nix() {
        assert_eq!(
            SkillLearner::classify_error("error: NixOS rebuild failed"),
            ErrorType::NixError
        );
    }

    #[test]
    fn test_classify_error_unknown() {
        assert_eq!(
            SkillLearner::classify_error("something went wrong"),
            ErrorType::Unknown
        );
    }

    #[test]
    fn test_build_search_query() {
        let query = SkillLearner::build_search_query(
            "nixops.rebuild",
            "build failed",
            Some("flake eval"),
        );
        assert!(query.contains("nixops.rebuild"));
        assert!(query.contains("build failed"));
        assert!(query.contains("flake eval"));
    }

    #[test]
    fn test_build_search_query_truncates() {
        let long_error = "a".repeat(300);
        let query = SkillLearner::build_search_query("action", &long_error, None);
        assert!(query.len() <= 200);
    }
}
