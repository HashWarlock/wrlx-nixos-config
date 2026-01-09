use super::types::Skill;

pub struct SkillMatcher {
    // TODO: Implement skill matching logic
}

impl SkillMatcher {
    pub fn new() -> Self {
        Self {}
    }

    /// Find skills that match the given query
    pub fn find_matching(&self, _query: &str, _skills: &[&Skill]) -> Vec<&Skill> {
        // TODO: Implement fuzzy matching on triggers
        Vec::new()
    }
}

impl Default for SkillMatcher {
    fn default() -> Self {
        Self::new()
    }
}
