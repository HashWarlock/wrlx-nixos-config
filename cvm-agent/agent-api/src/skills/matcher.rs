use super::types::Skill;

pub struct SkillMatcher;

#[derive(Debug, Clone)]
pub struct SkillMatch {
    pub skill_name: String,
    pub relevance: f32,
    pub matched_trigger: String,
}

impl SkillMatcher {
    /// Find skills matching the user input
    pub fn match_skills(input: &str, skills: &[&Skill]) -> Vec<SkillMatch> {
        let input_lower = input.to_lowercase();
        let input_words: Vec<&str> = input_lower.split_whitespace().collect();

        let mut matches: Vec<SkillMatch> = skills
            .iter()
            .filter_map(|skill| {
                let (relevance, matched_trigger) = Self::calculate_relevance(
                    &input_lower,
                    &input_words,
                    skill.triggers(),
                );

                if relevance > 0.0 {
                    Some(SkillMatch {
                        skill_name: skill.name().to_string(),
                        relevance,
                        matched_trigger,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by relevance descending
        matches.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());

        matches
    }

    fn calculate_relevance(
        input: &str,
        input_words: &[&str],
        triggers: &[String],
    ) -> (f32, String) {
        let mut best_relevance = 0.0f32;
        let mut best_trigger = String::new();

        for trigger in triggers {
            let trigger_lower = trigger.to_lowercase();
            let trigger_words: Vec<&str> = trigger_lower.split_whitespace().collect();

            // Exact match
            if input.contains(&trigger_lower) {
                let relevance = 1.0;
                if relevance > best_relevance {
                    best_relevance = relevance;
                    best_trigger = trigger.clone();
                }
                continue;
            }

            // Word overlap
            let matching_words = input_words
                .iter()
                .filter(|w| trigger_words.contains(w))
                .count();

            if matching_words > 0 {
                let relevance = matching_words as f32 / trigger_words.len().max(1) as f32;
                if relevance > best_relevance {
                    best_relevance = relevance;
                    best_trigger = trigger.clone();
                }
            }

            // Fuzzy match (simple Levenshtein-based)
            for trigger_word in &trigger_words {
                for input_word in input_words {
                    if Self::is_similar(input_word, trigger_word) {
                        let relevance = 0.5;
                        if relevance > best_relevance {
                            best_relevance = relevance;
                            best_trigger = trigger.clone();
                        }
                    }
                }
            }
        }

        (best_relevance, best_trigger)
    }

    fn is_similar(a: &str, b: &str) -> bool {
        if a == b {
            return true;
        }

        // Simple prefix match
        if a.len() >= 3 && b.starts_with(a) {
            return true;
        }
        if b.len() >= 3 && a.starts_with(b) {
            return true;
        }

        // Edit distance threshold
        let distance = Self::levenshtein(a, b);
        let max_len = a.len().max(b.len());
        let threshold = (max_len as f32 * 0.3).ceil() as usize;

        distance <= threshold
    }

    fn levenshtein(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();

        let mut dp = vec![vec![0; b_chars.len() + 1]; a_chars.len() + 1];

        for i in 0..=a_chars.len() {
            dp[i][0] = i;
        }
        for j in 0..=b_chars.len() {
            dp[0][j] = j;
        }

        for i in 1..=a_chars.len() {
            for j in 1..=b_chars.len() {
                let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
                dp[i][j] = (dp[i - 1][j] + 1)
                    .min(dp[i][j - 1] + 1)
                    .min(dp[i - 1][j - 1] + cost);
            }
        }

        dp[a_chars.len()][b_chars.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::types::InstructionSkill;

    fn make_skill(name: &str, triggers: Vec<&str>) -> Skill {
        Skill::Instruction(InstructionSkill {
            name: name.to_string(),
            description: String::new(),
            triggers: triggers.into_iter().map(|s| s.to_string()).collect(),
            content: String::new(),
            source: "test".to_string(),
        })
    }

    #[test]
    fn test_exact_match() {
        let skills = vec![
            make_skill("deploy", vec!["deploy", "push changes"]),
        ];
        let skill_refs: Vec<&Skill> = skills.iter().collect();

        let matches = SkillMatcher::match_skills("deploy", &skill_refs);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].skill_name, "deploy");
        assert_eq!(matches[0].relevance, 1.0);
    }

    #[test]
    fn test_partial_match() {
        let skills = vec![
            make_skill("nixos", vec!["edit config", "modify nix"]),
        ];
        let skill_refs: Vec<&Skill> = skills.iter().collect();

        let matches = SkillMatcher::match_skills("edit the config file", &skill_refs);
        assert!(!matches.is_empty());
        assert!(matches[0].relevance > 0.0);
    }
}
