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
