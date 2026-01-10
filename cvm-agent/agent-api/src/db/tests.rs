//! Integration tests for the memory system

use super::{init_memory_db, layer, MemoryRepository};

/// Integration test: Full memory lifecycle
/// Store → Query → Search → Pin → Forget candidates → Delete
#[test]
fn test_memory_lifecycle_integration() {
    let db = init_memory_db().unwrap();
    let repo = MemoryRepository::new(db);

    // 1. Store memories in different layers
    let working_id = repo
        .store(layer::WORKING, "Current task: implement feature X", "{}", None)
        .unwrap();

    let facts_id = repo
        .store(
            layer::FACTS,
            "NixOS version: 24.05",
            r#"{"key": "nixos_version"}"#,
            None,
        )
        .unwrap();

    let prefs_id = repo
        .store(
            layer::PREFERENCES,
            "User prefers dark theme",
            r#"{"category": "ui"}"#,
            None,
        )
        .unwrap();

    // Store an expired memory for forget testing
    let expired_id = repo
        .store(layer::WORKING, "Old task context", "{}", Some(-1000))
        .unwrap();

    // 2. Query all memories
    let all = repo.query(&[], 50).unwrap();
    assert_eq!(all.len(), 4, "Should have 4 memories");

    // 3. Query by layer
    let working_only = repo.query(&[layer::WORKING], 50).unwrap();
    assert_eq!(working_only.len(), 2, "Should have 2 working memories");

    let facts_only = repo.query(&[layer::FACTS], 50).unwrap();
    assert_eq!(facts_only.len(), 1, "Should have 1 fact");
    assert_eq!(facts_only[0].content, "NixOS version: 24.05");

    // 4. Full-text search
    let search_results = repo.search("NixOS", &[], 10).unwrap();
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].id, facts_id);

    let feature_search = repo.search("feature", &[], 10).unwrap();
    assert_eq!(feature_search.len(), 1);
    assert_eq!(feature_search[0].id, working_id);

    // 5. Pin a memory
    repo.pin(&prefs_id, true).unwrap();
    let pinned = repo.get_by_id(&prefs_id).unwrap().unwrap();
    assert!(pinned.pinned, "Memory should be pinned");

    // 6. Update access and verify
    let before = repo.get_by_id(&working_id).unwrap().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    repo.update_access(&working_id).unwrap();
    let after = repo.get_by_id(&working_id).unwrap().unwrap();
    assert!(
        after.last_accessed_ms > before.last_accessed_ms,
        "Last access time should increase"
    );
    assert_eq!(after.access_count, 1, "Access count should be 1");

    // 7. Get forget candidates (should find expired)
    let candidates = repo.get_forget_candidates().unwrap();
    assert!(
        candidates.iter().any(|c| c.id == expired_id),
        "Expired memory should be a forget candidate"
    );

    // 8. Delete expired (but not pinned)
    let deleted = repo.delete_many(&[expired_id.clone(), prefs_id.clone()]).unwrap();
    assert_eq!(deleted, 1, "Only unpinned expired should be deleted");

    // Verify expired is gone
    assert!(
        repo.get_by_id(&expired_id).unwrap().is_none(),
        "Expired should be deleted"
    );

    // Verify pinned is still there
    assert!(
        repo.get_by_id(&prefs_id).unwrap().is_some(),
        "Pinned should still exist"
    );

    // 9. Final count
    let final_count = repo.query(&[], 50).unwrap();
    assert_eq!(final_count.len(), 3, "Should have 3 memories remaining");
}

/// Integration test: Search across multiple layers
#[test]
fn test_cross_layer_search_integration() {
    let db = init_memory_db().unwrap();
    let repo = MemoryRepository::new(db);

    // Store related content in different layers
    repo.store(layer::WORKING, "Working on Hyprland config", "{}", None)
        .unwrap();
    repo.store(layer::FACTS, "Desktop: Hyprland", "{}", None)
        .unwrap();
    repo.store(
        layer::ARCHIVE,
        "Previous Hyprland troubleshooting session",
        "{}",
        None,
    )
    .unwrap();
    repo.store(layer::PREFERENCES, "User prefers i3-like keybindings", "{}", None)
        .unwrap();

    // Search for "Hyprland" across all layers
    let results = repo.search("Hyprland", &[], 10).unwrap();
    assert_eq!(results.len(), 3, "Should find Hyprland in 3 layers");

    // Search within specific layer
    let facts_results = repo.search("Hyprland", &[layer::FACTS], 10).unwrap();
    assert_eq!(facts_results.len(), 1);
    assert_eq!(facts_results[0].content, "Desktop: Hyprland");
}

/// Integration test: Forget candidate classification
#[test]
fn test_forget_candidate_classification_integration() {
    let db = init_memory_db().unwrap();
    let repo = MemoryRepository::new(db);

    // Create various memories
    let _active = repo
        .store(layer::WORKING, "Active task", "{}", None)
        .unwrap();

    let expired = repo
        .store(layer::WORKING, "Expired task", "{}", Some(-1000))
        .unwrap();

    let pinned = repo
        .store(layer::PREFERENCES, "Important preference", "{}", None)
        .unwrap();
    repo.pin(&pinned, true).unwrap();

    // Get candidates
    let candidates = repo.get_forget_candidates().unwrap();

    // Should include expired
    assert!(
        candidates.iter().any(|c| c.id == expired),
        "Expired should be candidate"
    );

    // Should NOT include pinned
    assert!(
        !candidates.iter().any(|c| c.id == pinned),
        "Pinned should not be candidate"
    );
}
