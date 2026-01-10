use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Error categories for recovery actions
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    /// NixOS rebuild/rollback errors
    NixBuild,
    /// Git operations failed
    Git,
    /// GUI automation errors
    GUI,
    /// Network/API errors
    Network,
    /// Database errors
    Database,
    /// Unknown/uncategorized
    Unknown,
}

impl ErrorCategory {
    /// Classify an error message into a category
    pub fn classify(error: &str) -> Self {
        let lower = error.to_lowercase();

        if lower.contains("nix") || lower.contains("rebuild") || lower.contains("derivation") {
            ErrorCategory::NixBuild
        } else if lower.contains("git") || lower.contains("commit") || lower.contains("push") {
            ErrorCategory::Git
        } else if lower.contains("atspi") || lower.contains("xdotool") || lower.contains("screenshot") {
            ErrorCategory::GUI
        } else if lower.contains("connection") || lower.contains("timeout") || lower.contains("network") {
            ErrorCategory::Network
        } else if lower.contains("database") || lower.contains("sqlite") || lower.contains("sql") {
            ErrorCategory::Database
        } else {
            ErrorCategory::Unknown
        }
    }
}

/// Suggested recovery action for an error
#[derive(Debug, Clone)]
pub struct RecoveryAction {
    pub description: String,
    pub command: Option<String>,
    pub is_destructive: bool,
}

/// Get suggested recovery actions for an error
pub fn get_recovery_actions(category: &ErrorCategory, error_message: &str) -> Vec<RecoveryAction> {
    match category {
        ErrorCategory::NixBuild => vec![
            RecoveryAction {
                description: "Rollback to previous generation".to_string(),
                command: Some("nixos-rebuild switch --rollback".to_string()),
                is_destructive: false,
            },
            RecoveryAction {
                description: "Check build logs for details".to_string(),
                command: Some("journalctl -xe".to_string()),
                is_destructive: false,
            },
            RecoveryAction {
                description: "Clear nix evaluation cache".to_string(),
                command: Some("rm -rf ~/.cache/nix/".to_string()),
                is_destructive: true,
            },
        ],

        ErrorCategory::Git => vec![
            RecoveryAction {
                description: "Reset to last commit".to_string(),
                command: Some("git reset --hard HEAD".to_string()),
                is_destructive: true,
            },
            RecoveryAction {
                description: "Stash uncommitted changes".to_string(),
                command: Some("git stash".to_string()),
                is_destructive: false,
            },
            RecoveryAction {
                description: "Check remote status".to_string(),
                command: Some("git remote -v && git fetch --dry-run".to_string()),
                is_destructive: false,
            },
        ],

        ErrorCategory::GUI => vec![
            RecoveryAction {
                description: "Restart accessibility services".to_string(),
                command: Some("systemctl --user restart at-spi-dbus-bus".to_string()),
                is_destructive: false,
            },
            RecoveryAction {
                description: "Take a fresh screenshot".to_string(),
                command: None,
                is_destructive: false,
            },
        ],

        ErrorCategory::Network => {
            let mut actions = vec![
                RecoveryAction {
                    description: "Check network connectivity".to_string(),
                    command: Some("ping -c 3 8.8.8.8".to_string()),
                    is_destructive: false,
                },
            ];

            if error_message.contains("timeout") {
                actions.push(RecoveryAction {
                    description: "Retry with longer timeout".to_string(),
                    command: None,
                    is_destructive: false,
                });
            }

            actions
        }

        ErrorCategory::Database => vec![
            RecoveryAction {
                description: "Check database integrity".to_string(),
                command: Some("sqlite3 ./data/agent.db 'PRAGMA integrity_check;'".to_string()),
                is_destructive: false,
            },
            RecoveryAction {
                description: "Vacuum database".to_string(),
                command: Some("sqlite3 ./data/agent.db 'VACUUM;'".to_string()),
                is_destructive: false,
            },
        ],

        ErrorCategory::Unknown => vec![
            RecoveryAction {
                description: "View recent logs".to_string(),
                command: Some("journalctl -n 50".to_string()),
                is_destructive: false,
            },
        ],
    }
}

/// Track errors for circuit breaker pattern
#[derive(Debug)]
pub struct ErrorTracker {
    /// Recent errors with timestamps
    recent_errors: VecDeque<(Instant, ErrorCategory)>,
    /// Time window for counting errors
    window: Duration,
    /// Threshold before circuit opens
    threshold: usize,
}

impl ErrorTracker {
    pub fn new(window_secs: u64, threshold: usize) -> Self {
        Self {
            recent_errors: VecDeque::new(),
            window: Duration::from_secs(window_secs),
            threshold,
        }
    }

    /// Record an error occurrence
    pub fn record(&mut self, category: ErrorCategory) {
        let now = Instant::now();
        self.recent_errors.push_back((now, category));
        self.prune_old();
    }

    /// Remove errors outside the time window
    fn prune_old(&mut self) {
        let cutoff = Instant::now() - self.window;
        while let Some((time, _)) = self.recent_errors.front() {
            if *time < cutoff {
                self.recent_errors.pop_front();
            } else {
                break;
            }
        }
    }

    /// Check if circuit should be open (too many errors)
    pub fn is_circuit_open(&mut self) -> bool {
        self.prune_old();
        self.recent_errors.len() >= self.threshold
    }

    /// Get count of errors by category in current window
    pub fn count_by_category(&mut self, category: &ErrorCategory) -> usize {
        self.prune_old();
        self.recent_errors
            .iter()
            .filter(|(_, c)| c == category)
            .count()
    }

    /// Clear all tracked errors (circuit reset)
    pub fn reset(&mut self) {
        self.recent_errors.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_category_classification() {
        assert_eq!(
            ErrorCategory::classify("NixOS rebuild failed"),
            ErrorCategory::NixBuild
        );
        assert_eq!(
            ErrorCategory::classify("git push rejected"),
            ErrorCategory::Git
        );
        assert_eq!(
            ErrorCategory::classify("atspi element not found"),
            ErrorCategory::GUI
        );
        assert_eq!(
            ErrorCategory::classify("connection timeout"),
            ErrorCategory::Network
        );
        assert_eq!(
            ErrorCategory::classify("sqlite constraint violation"),
            ErrorCategory::Database
        );
        assert_eq!(
            ErrorCategory::classify("something random"),
            ErrorCategory::Unknown
        );
    }

    #[test]
    fn test_recovery_actions() {
        let actions = get_recovery_actions(&ErrorCategory::NixBuild, "build failed");
        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| a.description.contains("Rollback")));
    }

    #[test]
    fn test_error_tracker_circuit() {
        let mut tracker = ErrorTracker::new(60, 3);

        assert!(!tracker.is_circuit_open());

        tracker.record(ErrorCategory::Network);
        tracker.record(ErrorCategory::Network);
        assert!(!tracker.is_circuit_open());

        tracker.record(ErrorCategory::Network);
        assert!(tracker.is_circuit_open());

        tracker.reset();
        assert!(!tracker.is_circuit_open());
    }

    #[test]
    fn test_count_by_category() {
        let mut tracker = ErrorTracker::new(60, 10);

        tracker.record(ErrorCategory::Network);
        tracker.record(ErrorCategory::Network);
        tracker.record(ErrorCategory::Git);
        tracker.record(ErrorCategory::Network);

        assert_eq!(tracker.count_by_category(&ErrorCategory::Network), 3);
        assert_eq!(tracker.count_by_category(&ErrorCategory::Git), 1);
        assert_eq!(tracker.count_by_category(&ErrorCategory::NixBuild), 0);
    }

    #[test]
    fn test_destructive_actions_flagged() {
        let nix_actions = get_recovery_actions(&ErrorCategory::NixBuild, "");
        let destructive: Vec<_> = nix_actions.iter().filter(|a| a.is_destructive).collect();
        assert!(!destructive.is_empty());

        let git_actions = get_recovery_actions(&ErrorCategory::Git, "");
        let git_destructive: Vec<_> = git_actions.iter().filter(|a| a.is_destructive).collect();
        assert!(!git_destructive.is_empty());
    }
}
