use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillType {
    Instruction,
    Workflow,
}

#[derive(Debug, Clone)]
pub enum Skill {
    Instruction(InstructionSkill),
    Workflow(WorkflowSkill),
}

impl Skill {
    pub fn name(&self) -> &str {
        match self {
            Skill::Instruction(s) => &s.name,
            Skill::Workflow(s) => &s.name,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Skill::Instruction(s) => &s.description,
            Skill::Workflow(s) => &s.description,
        }
    }

    pub fn triggers(&self) -> &[String] {
        match self {
            Skill::Instruction(s) => &s.triggers,
            Skill::Workflow(s) => &s.triggers,
        }
    }

    pub fn source(&self) -> &str {
        match self {
            Skill::Instruction(s) => &s.source,
            Skill::Workflow(s) => &s.source,
        }
    }

    #[allow(dead_code)]
    pub fn skill_type(&self) -> SkillType {
        match self {
            Skill::Instruction(_) => SkillType::Instruction,
            Skill::Workflow(_) => SkillType::Workflow,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionSkill {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub content: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSkill {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub parameters: Vec<WorkflowParameter>,
    pub steps: Vec<WorkflowStep>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub action: String,
    pub description: Option<String>,
    pub condition: Option<String>,
    pub args: Option<serde_json::Value>,
    pub stream: Option<bool>,
    pub message: Option<String>,
}
