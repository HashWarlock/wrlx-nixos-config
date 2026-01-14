use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

use super::types::{InstructionSkill, Skill, WorkflowSkill, WorkflowParameter, WorkflowStep};

pub struct SkillsLoader {
    skills_dir: String,
    skills: HashMap<String, Skill>,
}

impl SkillsLoader {
    pub fn new(skills_dir: &str) -> Self {
        Self {
            skills_dir: skills_dir.to_string(),
            skills: HashMap::new(),
        }
    }

    pub async fn load_all(&mut self) -> Result<()> {
        self.skills.clear();
        let mut warning_count = 0;

        let instructions_dir = format!("{}/instructions", self.skills_dir);
        if Path::new(&instructions_dir).exists() {
            warning_count += self.load_instruction_skills(&instructions_dir).await?;
        }

        let workflows_dir = format!("{}/workflows", self.skills_dir);
        if Path::new(&workflows_dir).exists() {
            warning_count += self.load_workflow_skills(&workflows_dir).await?;
        }

        if warning_count > 0 {
            tracing::info!("Loaded {} skills ({} warnings)", self.skills.len(), warning_count);
        } else {
            tracing::info!("Loaded {} skills", self.skills.len());
        }
        Ok(())
    }

    async fn load_instruction_skills(&mut self, dir: &str) -> Result<usize> {
        let mut entries = fs::read_dir(dir).await?;
        let mut warning_count = 0;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                match self.parse_instruction_skill(&path).await {
                    Ok(skill) => {
                        self.skills.insert(skill.name.clone(), Skill::Instruction(skill));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse instruction skill '{}': {}", path.display(), e);
                        warning_count += 1;
                    }
                }
            }
        }

        Ok(warning_count)
    }

    async fn load_workflow_skills(&mut self, dir: &str) -> Result<usize> {
        let mut entries = fs::read_dir(dir).await?;
        let mut warning_count = 0;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
                match self.parse_workflow_skill(&path).await {
                    Ok(skill) => {
                        self.skills.insert(skill.name.clone(), Skill::Workflow(skill));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse workflow skill '{}': {}", path.display(), e);
                        warning_count += 1;
                    }
                }
            }
        }

        Ok(warning_count)
    }

    async fn parse_instruction_skill(&self, path: &Path) -> Result<InstructionSkill> {
        let content = fs::read_to_string(path).await?;
        let (frontmatter, body) = Self::split_frontmatter(&content)?;

        #[derive(serde::Deserialize)]
        struct Frontmatter {
            name: String,
            description: String,
            triggers: Vec<String>,
        }

        let fm: Frontmatter = serde_yaml::from_str(&frontmatter)?;

        Ok(InstructionSkill {
            name: fm.name,
            description: fm.description,
            triggers: fm.triggers,
            content: body,
            source: "builtin".to_string(),
        })
    }

    async fn parse_workflow_skill(&self, path: &Path) -> Result<WorkflowSkill> {
        let content = fs::read_to_string(path).await?;

        #[derive(serde::Deserialize)]
        struct RawWorkflow {
            name: String,
            description: String,
            triggers: Vec<String>,
            #[serde(default)]
            parameters: HashMap<String, RawParameter>,
            steps: Vec<RawStep>,
        }

        #[derive(serde::Deserialize)]
        struct RawParameter {
            description: String,
            #[serde(default)]
            required: bool,
            default: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct RawStep {
            id: String,
            action: String,
            description: Option<String>,
            condition: Option<String>,
            args: Option<serde_json::Value>,
            stream: Option<bool>,
            message: Option<String>,
        }

        let raw: RawWorkflow = serde_yaml::from_str(&content)?;

        let parameters = raw.parameters
            .into_iter()
            .map(|(name, p)| WorkflowParameter {
                name,
                description: p.description,
                required: p.required,
                default: p.default,
            })
            .collect();

        let steps = raw.steps
            .into_iter()
            .map(|s| WorkflowStep {
                id: s.id,
                action: s.action,
                description: s.description,
                condition: s.condition,
                args: s.args,
                stream: s.stream,
                message: s.message,
            })
            .collect();

        Ok(WorkflowSkill {
            name: raw.name,
            description: raw.description,
            triggers: raw.triggers,
            parameters,
            steps,
            source: "builtin".to_string(),
        })
    }

    fn split_frontmatter(content: &str) -> Result<(String, String)> {
        let content = content.trim();
        if !content.starts_with("---") {
            anyhow::bail!("No frontmatter found");
        }

        let rest = &content[3..];
        let end = rest.find("---").ok_or_else(|| anyhow::anyhow!("No frontmatter end"))?;

        let frontmatter = rest[..end].trim().to_string();
        let body = rest[end + 3..].trim().to_string();

        Ok((frontmatter, body))
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    #[allow(dead_code)]
    pub fn list_by_type(&self, skill_type: super::types::SkillType) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| matches!(
                (s, &skill_type),
                (Skill::Instruction(_), super::types::SkillType::Instruction) |
                (Skill::Workflow(_), super::types::SkillType::Workflow)
            ))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Test 1: Handle missing/empty skills directory gracefully
    #[tokio::test]
    async fn test_load_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut loader = SkillsLoader::new(temp_dir.path().to_str().unwrap());

        // Should succeed with no skills loaded (no instructions/ or workflows/ subdirs)
        let result = loader.load_all().await;
        assert!(result.is_ok(), "load_all should succeed on empty directory");
        assert_eq!(loader.list().len(), 0, "Should have 0 skills loaded");

        // Create empty subdirectories
        let instructions_dir = temp_dir.path().join("instructions");
        let workflows_dir = temp_dir.path().join("workflows");
        fs::create_dir_all(&instructions_dir).await.unwrap();
        fs::create_dir_all(&workflows_dir).await.unwrap();

        // Should still succeed with 0 skills
        let result = loader.load_all().await;
        assert!(result.is_ok(), "load_all should succeed with empty subdirs");
        assert_eq!(loader.list().len(), 0, "Should have 0 skills loaded");
    }

    /// Test 2: Valid markdown with frontmatter parses correctly
    #[tokio::test]
    async fn test_parse_instruction_skill_valid() {
        let temp_dir = TempDir::new().unwrap();
        let instructions_dir = temp_dir.path().join("instructions");
        fs::create_dir_all(&instructions_dir).await.unwrap();

        let valid_skill = r#"---
name: test-skill
description: A test skill for validation
triggers:
  - test this
  - run test
---

# Test Skill Instructions

This is the body content of the skill.

## Steps

1. Do something
2. Do something else
"#;

        let skill_path = instructions_dir.join("test-skill.md");
        fs::write(&skill_path, valid_skill).await.unwrap();

        let mut loader = SkillsLoader::new(temp_dir.path().to_str().unwrap());
        let result = loader.load_all().await;
        assert!(result.is_ok(), "load_all should succeed");
        assert_eq!(loader.list().len(), 1, "Should have 1 skill loaded");

        let skill = loader.get("test-skill");
        assert!(skill.is_some(), "Should find skill by name");

        let skill = skill.unwrap();
        assert_eq!(skill.name(), "test-skill");
        assert_eq!(skill.description(), "A test skill for validation");
        assert_eq!(skill.triggers(), &["test this", "run test"]);

        // Verify content body
        if let Skill::Instruction(inst) = skill {
            assert!(inst.content.contains("# Test Skill Instructions"));
            assert!(inst.content.contains("Do something else"));
        } else {
            panic!("Expected Instruction skill");
        }
    }

    /// Test 3: Malformed frontmatter handling
    #[tokio::test]
    async fn test_parse_instruction_skill_malformed() {
        let temp_dir = TempDir::new().unwrap();
        let instructions_dir = temp_dir.path().join("instructions");
        fs::create_dir_all(&instructions_dir).await.unwrap();

        // Case 1: No frontmatter at all
        let no_frontmatter = r#"# Just Markdown

No frontmatter here, just content.
"#;
        fs::write(instructions_dir.join("no-frontmatter.md"), no_frontmatter)
            .await
            .unwrap();

        // Case 2: Frontmatter without closing ---
        let no_closing = r#"---
name: incomplete
description: Missing closing delimiter
triggers:
  - trigger

This content looks like body but frontmatter never closed.
"#;
        fs::write(instructions_dir.join("no-closing.md"), no_closing)
            .await
            .unwrap();

        // Case 3: Valid frontmatter structure but missing required field (name)
        let missing_name = r#"---
description: Has description but no name
triggers:
  - trigger
---

Body content here.
"#;
        fs::write(instructions_dir.join("missing-name.md"), missing_name)
            .await
            .unwrap();

        // Case 4: Valid frontmatter structure but missing required field (triggers)
        let missing_triggers = r#"---
name: no-triggers
description: Has name but no triggers
---

Body content here.
"#;
        fs::write(instructions_dir.join("missing-triggers.md"), missing_triggers)
            .await
            .unwrap();

        let mut loader = SkillsLoader::new(temp_dir.path().to_str().unwrap());
        let result = loader.load_all().await;

        // Should succeed overall but with warnings for each malformed file
        assert!(result.is_ok(), "load_all should succeed despite malformed files");
        assert_eq!(loader.list().len(), 0, "No valid skills should be loaded");
    }

    /// Test 4: Edge cases for split_frontmatter function
    #[tokio::test]
    async fn test_split_frontmatter_edge_cases() {
        // Case 1: Empty string
        let result = SkillsLoader::split_frontmatter("");
        assert!(result.is_err(), "Empty string should error");
        assert!(
            result.unwrap_err().to_string().contains("No frontmatter found"),
            "Should indicate no frontmatter"
        );

        // Case 2: Only opening "---"
        let result = SkillsLoader::split_frontmatter("---");
        assert!(result.is_err(), "Only opening --- should error");
        assert!(
            result.unwrap_err().to_string().contains("No frontmatter end"),
            "Should indicate no end delimiter"
        );

        // Case 3: "---\n---" - empty frontmatter and body (valid structure)
        let result = SkillsLoader::split_frontmatter("---\n---");
        assert!(result.is_ok(), "Empty frontmatter should be valid structure");
        let (frontmatter, body) = result.unwrap();
        assert_eq!(frontmatter, "", "Frontmatter should be empty");
        assert_eq!(body, "", "Body should be empty");

        // Case 4: Content without any "---"
        let result = SkillsLoader::split_frontmatter("Just plain text without delimiters");
        assert!(result.is_err(), "No delimiters should error");
        assert!(
            result.unwrap_err().to_string().contains("No frontmatter found"),
            "Should indicate no frontmatter"
        );

        // Case 5: Valid frontmatter with content
        let result = SkillsLoader::split_frontmatter("---\nkey: value\n---\nBody text");
        assert!(result.is_ok(), "Valid frontmatter should parse");
        let (frontmatter, body) = result.unwrap();
        assert_eq!(frontmatter, "key: value");
        assert_eq!(body, "Body text");

        // Case 6: Frontmatter with leading whitespace (trim() handles this)
        // The function trims input first, so "  ---\nkey: value\n---" becomes "---\nkey: value\n---"
        let result = SkillsLoader::split_frontmatter("  ---\nkey: value\n---\nBody");
        assert!(result.is_ok(), "Leading whitespace should be trimmed and parse OK");
        let (frontmatter, body) = result.unwrap();
        assert_eq!(frontmatter, "key: value");
        assert_eq!(body, "Body");

        // Case 7: Multiple --- in body (should only split on first closing ---)
        let result = SkillsLoader::split_frontmatter("---\nfm: data\n---\nBody with --- in it");
        assert!(result.is_ok());
        let (frontmatter, body) = result.unwrap();
        assert_eq!(frontmatter, "fm: data");
        assert_eq!(body, "Body with --- in it");
    }

    /// Test 5: Duplicate skill names - HashMap behavior (last one wins)
    #[tokio::test]
    async fn test_duplicate_skill_names() {
        let temp_dir = TempDir::new().unwrap();
        let instructions_dir = temp_dir.path().join("instructions");
        fs::create_dir_all(&instructions_dir).await.unwrap();

        // Create two files with the same skill name
        let skill_v1 = r#"---
name: duplicate-skill
description: Version 1 of the skill
triggers:
  - trigger v1
---

Content version 1
"#;

        let skill_v2 = r#"---
name: duplicate-skill
description: Version 2 of the skill
triggers:
  - trigger v2
---

Content version 2
"#;

        // Use different filenames but same skill name in frontmatter
        fs::write(instructions_dir.join("skill-a.md"), skill_v1)
            .await
            .unwrap();
        fs::write(instructions_dir.join("skill-b.md"), skill_v2)
            .await
            .unwrap();

        let mut loader = SkillsLoader::new(temp_dir.path().to_str().unwrap());
        let result = loader.load_all().await;
        assert!(result.is_ok(), "load_all should succeed");

        // HashMap means only one skill with that name is stored (the last one processed)
        assert_eq!(loader.list().len(), 1, "Should have only 1 skill (duplicates overwritten)");

        let skill = loader.get("duplicate-skill");
        assert!(skill.is_some(), "Should find the skill by name");

        // Note: The order of file processing is not guaranteed, so we just verify
        // that one of the two versions is present, not which specific one
        let skill = skill.unwrap();
        assert_eq!(skill.name(), "duplicate-skill");
        let desc = skill.description();
        assert!(
            desc == "Version 1 of the skill" || desc == "Version 2 of the skill",
            "Should have one of the two descriptions"
        );
    }
}
