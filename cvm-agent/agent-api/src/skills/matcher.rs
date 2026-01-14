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

    #[test]
    fn test_fuzzy_match_levenshtein() {
        // Test Levenshtein distance function directly
        assert_eq!(SkillMatcher::levenshtein("deploy", "deploy"), 0);
        assert_eq!(SkillMatcher::levenshtein("deploye", "deploy"), 1); // 1 char extra
        assert_eq!(SkillMatcher::levenshtein("dploy", "deploy"), 1); // 1 char missing
        assert_eq!(SkillMatcher::levenshtein("deplyo", "deploy"), 2); // transposition = 2 ops
        assert_eq!(SkillMatcher::levenshtein("", "deploy"), 6); // empty to full
        assert_eq!(SkillMatcher::levenshtein("deploy", ""), 6); // full to empty

        // Test fuzzy matching through match_skills
        let skills = vec![make_skill("deploy", vec!["deploy"])];
        let skill_refs: Vec<&Skill> = skills.iter().collect();

        // "deploye" should match "deploy" (1 char typo within threshold)
        let matches = SkillMatcher::match_skills("deploye", &skill_refs);
        assert!(!matches.is_empty(), "deploye should fuzzy match deploy");
        assert!(matches[0].relevance > 0.0);

        // "dploy" should match "deploy" (1 char missing within threshold)
        let matches = SkillMatcher::match_skills("dploy", &skill_refs);
        assert!(!matches.is_empty(), "dploy should fuzzy match deploy");
        assert!(matches[0].relevance > 0.0);

        // "completely different" should NOT match "deploy"
        let matches = SkillMatcher::match_skills("completely different", &skill_refs);
        assert!(
            matches.is_empty(),
            "completely different should not match deploy"
        );
    }

    #[test]
    fn test_empty_input() {
        let skills = vec![make_skill("deploy", vec!["deploy", "push changes"])];
        let skill_refs: Vec<&Skill> = skills.iter().collect();

        // Empty input string should return no matches
        let matches = SkillMatcher::match_skills("", &skill_refs);
        assert!(matches.is_empty(), "empty input should return no matches");

        // Whitespace-only input should return no matches
        let matches = SkillMatcher::match_skills("   ", &skill_refs);
        assert!(
            matches.is_empty(),
            "whitespace-only input should return no matches"
        );

        // Empty skills list should return no matches
        let empty_skills: Vec<&Skill> = vec![];
        let matches = SkillMatcher::match_skills("deploy", &empty_skills);
        assert!(
            matches.is_empty(),
            "empty skills list should return no matches"
        );

        // Skill with empty triggers should be skipped
        let skill_empty_triggers = make_skill("empty", vec![]);
        let skills_with_empty = vec![&skill_empty_triggers];
        let matches = SkillMatcher::match_skills("deploy", &skills_with_empty);
        assert!(
            matches.is_empty(),
            "skill with empty triggers should not match"
        );
    }

    #[test]
    fn test_unicode_matching() {
        // Test with accented characters
        let skills_accent = vec![make_skill("cafe", vec!["cafe", "coffee"])];
        let skill_refs: Vec<&Skill> = skills_accent.iter().collect();

        // Exact match with accent (cafe matches cafe)
        let matches = SkillMatcher::match_skills("cafe", &skill_refs);
        assert!(!matches.is_empty(), "cafe should match cafe trigger");

        // Note: cafe (with accent) won't match cafe (without) unless normalized
        // This tests current behavior - accent-aware matching would be an enhancement
        let _matches = SkillMatcher::match_skills("café", &skill_refs);
        // Current impl treats these as different - fuzzy match may still work
        // due to Levenshtein distance being 1

        // Test with emoji in input - should not crash
        let matches = SkillMatcher::match_skills("deploy 🚀", &skill_refs);
        // Should handle gracefully (emoji won't match, but no panic)
        assert!(matches.is_empty() || matches[0].relevance >= 0.0);

        // Test with CJK characters
        let skills_cjk = vec![make_skill("japanese", vec!["日本語", "nihongo"])];
        let skill_refs_cjk: Vec<&Skill> = skills_cjk.iter().collect();

        // Exact CJK match
        let matches = SkillMatcher::match_skills("日本語", &skill_refs_cjk);
        assert!(!matches.is_empty(), "CJK characters should match exactly");
        assert_eq!(matches[0].relevance, 1.0);

        // Partial CJK should not panic
        let matches = SkillMatcher::match_skills("日本", &skill_refs_cjk);
        // May or may not match depending on impl, but should not crash
        assert!(matches.is_empty() || matches[0].relevance >= 0.0);
    }

    #[test]
    fn test_very_long_strings() {
        use std::time::Instant;

        // Create a skill with reasonable triggers
        let skills = vec![make_skill("deploy", vec!["deploy", "push changes"])];
        let skill_refs: Vec<&Skill> = skills.iter().collect();

        // Test with very long input (1000+ chars)
        let long_input = "deploy ".repeat(200); // 1400 chars
        let start = Instant::now();
        let matches = SkillMatcher::match_skills(&long_input, &skill_refs);
        let duration = start.elapsed();

        assert!(
            duration.as_secs() < 5,
            "long input should complete in reasonable time"
        );
        assert!(!matches.is_empty(), "long input with deploy should match");

        // Test with many skills (100+)
        let many_skills: Vec<Skill> = (0..100)
            .map(|i| make_skill(&format!("skill_{:03}", i), vec![&format!("trigger_{:03}", i)]))
            .collect();
        let many_skill_refs: Vec<&Skill> = many_skills.iter().collect();

        let start = Instant::now();
        let matches = SkillMatcher::match_skills("trigger_050", &many_skill_refs);
        let duration = start.elapsed();

        assert!(
            duration.as_secs() < 5,
            "many skills should complete in reasonable time"
        );
        assert!(!matches.is_empty(), "should find skill_050");
        assert_eq!(matches[0].skill_name, "skill_050");

        // Test with both long input and many skills
        let start = Instant::now();
        let matches = SkillMatcher::match_skills(&long_input, &many_skill_refs);
        let duration = start.elapsed();

        assert!(
            duration.as_secs() < 10,
            "long input + many skills should complete in reasonable time"
        );
        // May or may not match, but should not timeout
        assert!(matches.is_empty() || matches[0].relevance >= 0.0);
    }
}
