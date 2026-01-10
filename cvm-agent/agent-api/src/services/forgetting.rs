use crate::db::{layer, DbPool, MemoryRepository};
use chrono::Utc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Triggers that initiate forgetting actions
#[derive(Debug, Clone)]
pub enum ForgettingTrigger {
    /// NixOS rebuild completed (success triggers cleanup)
    NixosRebuild { success: bool },
    /// Git commit was made
    GitCommit { hash: String },
    /// Git push completed
    GitPush { branch: String },
    /// An error was resolved
    ErrorResolved { error_id: String },
    /// A preference was contradicted
    PreferenceContradicted { preference_id: String, count: i32 },
    /// A system fact became stale
    SystemFactStale { fact_key: String },
    /// Scheduled cleanup sweep
    ScheduledSweep,
}

/// Actions to take in response to triggers
#[derive(Debug, Clone, PartialEq)]
pub enum ForgettingAction {
    /// Remove memories from previous NixOS generation
    PurgePreviousGeneration,
    /// Archive pre-commit working context
    ArchivePreCommitState,
    /// Summarize and archive error context
    SummarizeAndArchiveError,
    /// Remove a contradicted preference
    RemovePreference,
    /// Refresh a stale system fact
    RefreshFact,
    /// Queue items for user confirmation
    QueueForUserConfirmation,
    /// Auto-delete expired entries
    AutoDelete,
}

impl ForgettingTrigger {
    /// Get the actions to take for this trigger
    pub fn get_actions(&self) -> Vec<ForgettingAction> {
        match self {
            ForgettingTrigger::NixosRebuild { success: true } => {
                vec![ForgettingAction::PurgePreviousGeneration]
            }
            ForgettingTrigger::NixosRebuild { success: false } => vec![],
            ForgettingTrigger::GitCommit { .. } | ForgettingTrigger::GitPush { .. } => {
                vec![ForgettingAction::ArchivePreCommitState]
            }
            ForgettingTrigger::ErrorResolved { .. } => {
                vec![ForgettingAction::SummarizeAndArchiveError]
            }
            ForgettingTrigger::PreferenceContradicted { count, .. } if *count >= 3 => {
                vec![ForgettingAction::RemovePreference]
            }
            ForgettingTrigger::PreferenceContradicted { .. } => vec![],
            ForgettingTrigger::SystemFactStale { .. } => {
                vec![ForgettingAction::RefreshFact]
            }
            ForgettingTrigger::ScheduledSweep => {
                vec![
                    ForgettingAction::AutoDelete,
                    ForgettingAction::QueueForUserConfirmation,
                ]
            }
        }
    }
}

/// Determine if a memory should be auto-forgotten without user confirmation
pub fn should_auto_forget(layer_id: i32, pinned: bool, expired: bool) -> bool {
    if pinned {
        return false;
    }

    match layer_id {
        l if l == layer::WORKING => expired,      // Working context can auto-expire
        l if l == layer::FACTS => expired,        // Stale facts can auto-refresh
        l if l == layer::ARCHIVE => false,        // Archive needs confirmation
        l if l == layer::PREFERENCES => false,    // Preferences need confirmation
        _ => expired,
    }
}

/// Background task that handles forgetting triggers
pub struct ForgettingManager {
    repo: MemoryRepository,
    trigger_rx: mpsc::Receiver<ForgettingTrigger>,
}

impl ForgettingManager {
    pub fn new(db: DbPool, trigger_rx: mpsc::Receiver<ForgettingTrigger>) -> Self {
        Self {
            repo: MemoryRepository::new(db),
            trigger_rx,
        }
    }

    /// Run the forgetting manager as a background task
    pub async fn run(mut self) {
        info!("Forgetting manager started");

        while let Some(trigger) = self.trigger_rx.recv().await {
            debug!("Received forgetting trigger: {:?}", trigger);
            let actions = trigger.get_actions();

            for action in actions {
                if let Err(e) = self.execute_action(&trigger, &action).await {
                    warn!("Forgetting action {:?} failed: {}", action, e);
                }
            }
        }

        info!("Forgetting manager stopped");
    }

    async fn execute_action(
        &self,
        trigger: &ForgettingTrigger,
        action: &ForgettingAction,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match action {
            ForgettingAction::PurgePreviousGeneration => {
                info!("Purging previous NixOS generation memory");
                // Delete memories with metadata containing old generation info
                let candidates = self.repo.get_forget_candidates()?;
                let to_delete: Vec<_> = candidates
                    .iter()
                    .filter(|m| m.metadata_json.contains("generation"))
                    .map(|m| m.id.clone())
                    .collect();

                if !to_delete.is_empty() {
                    let deleted = self.repo.delete_many(&to_delete)?;
                    info!("Purged {} generation-related memories", deleted);
                }
            }

            ForgettingAction::ArchivePreCommitState => {
                info!("Archiving pre-commit state");
                // In a full implementation, this would:
                // 1. Summarize current working context
                // 2. Move to archive layer
                // 3. Clear working context
            }

            ForgettingAction::SummarizeAndArchiveError => {
                if let ForgettingTrigger::ErrorResolved { error_id } = trigger {
                    info!("Archiving resolved error: {}", error_id);
                    // In a full implementation, summarize error context and move to archive
                }
            }

            ForgettingAction::RemovePreference => {
                if let ForgettingTrigger::PreferenceContradicted { preference_id, count } = trigger {
                    info!("Removing contradicted preference {} (contradicted {} times)", preference_id, count);
                    self.repo.delete_many(&[preference_id.clone()])?;
                }
            }

            ForgettingAction::RefreshFact => {
                if let ForgettingTrigger::SystemFactStale { fact_key } = trigger {
                    info!("Refreshing stale fact: {}", fact_key);
                    // In a full implementation, re-query the system and update the fact
                }
            }

            ForgettingAction::QueueForUserConfirmation => {
                debug!("Queuing candidates for user confirmation");
                // Candidates are already available via GetForgetCandidates RPC
                // This action just logs that there are items pending
                let candidates = self.repo.get_forget_candidates()?;
                if !candidates.is_empty() {
                    info!("{} memories pending user review for deletion", candidates.len());
                }
            }

            ForgettingAction::AutoDelete => {
                info!("Auto-deleting expired entries");
                let candidates = self.repo.get_forget_candidates()?;
                let now_ms = Utc::now().timestamp_millis();

                let to_delete: Vec<_> = candidates
                    .iter()
                    .filter(|m| {
                        let expired = m.expires_at_ms.map(|e| e < now_ms).unwrap_or(false);
                        should_auto_forget(m.layer, m.pinned, expired)
                    })
                    .map(|m| m.id.clone())
                    .collect();

                if !to_delete.is_empty() {
                    let deleted = self.repo.delete_many(&to_delete)?;
                    info!("Auto-deleted {} expired memories", deleted);
                }
            }
        }

        Ok(())
    }
}

/// Create a channel for sending forgetting triggers
pub fn create_trigger_channel() -> (mpsc::Sender<ForgettingTrigger>, mpsc::Receiver<ForgettingTrigger>) {
    mpsc::channel(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forgetting_trigger_rebuild_success() {
        let trigger = ForgettingTrigger::NixosRebuild { success: true };
        let actions = trigger.get_actions();
        assert!(actions.contains(&ForgettingAction::PurgePreviousGeneration));
    }

    #[test]
    fn test_forgetting_trigger_rebuild_failure() {
        let trigger = ForgettingTrigger::NixosRebuild { success: false };
        let actions = trigger.get_actions();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_forgetting_trigger_git_commit() {
        let trigger = ForgettingTrigger::GitCommit { hash: "abc123".into() };
        let actions = trigger.get_actions();
        assert!(actions.contains(&ForgettingAction::ArchivePreCommitState));
    }

    #[test]
    fn test_forgetting_trigger_preference_below_threshold() {
        let trigger = ForgettingTrigger::PreferenceContradicted {
            preference_id: "pref1".into(),
            count: 2,
        };
        let actions = trigger.get_actions();
        assert!(actions.is_empty()); // Not enough contradictions
    }

    #[test]
    fn test_forgetting_trigger_preference_at_threshold() {
        let trigger = ForgettingTrigger::PreferenceContradicted {
            preference_id: "pref1".into(),
            count: 3,
        };
        let actions = trigger.get_actions();
        assert!(actions.contains(&ForgettingAction::RemovePreference));
    }

    #[test]
    fn test_auto_forget_classification() {
        // Working layer, not pinned, expired -> auto-forget
        assert!(should_auto_forget(layer::WORKING, false, true));

        // Working layer, not pinned, not expired -> no auto-forget
        assert!(!should_auto_forget(layer::WORKING, false, false));

        // Preferences, not pinned, expired -> still needs confirmation
        assert!(!should_auto_forget(layer::PREFERENCES, false, true));

        // Archive, not pinned, expired -> still needs confirmation
        assert!(!should_auto_forget(layer::ARCHIVE, false, true));

        // Any layer, pinned -> never auto-forget
        assert!(!should_auto_forget(layer::WORKING, true, true));
        assert!(!should_auto_forget(layer::FACTS, true, true));
    }

    #[test]
    fn test_scheduled_sweep_actions() {
        let trigger = ForgettingTrigger::ScheduledSweep;
        let actions = trigger.get_actions();
        assert!(actions.contains(&ForgettingAction::AutoDelete));
        assert!(actions.contains(&ForgettingAction::QueueForUserConfirmation));
    }
}
