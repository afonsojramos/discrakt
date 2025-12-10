// Tests for AppState in src/state.rs

use discrakt::state::{AppState, WatchingInfo};

#[test]
fn test_app_state_default() {
    let state = AppState::default();

    assert!(state.current_watching.is_none());
    assert!(!state.discord_connected);
    assert!(!state.is_paused);
}

#[test]
fn test_app_state_new_creates_arc() {
    let state = AppState::new();

    // Verify we can acquire a read lock
    let guard = state.read().unwrap();
    assert!(guard.current_watching.is_none());
    assert!(!guard.discord_connected);
    assert!(!guard.is_paused);
}

#[test]
fn test_set_watching() {
    let mut state = AppState::default();

    state.set_watching(
        "Test Movie".to_string(),
        "Action".to_string(),
        "45.00%".to_string(),
    );

    assert!(state.current_watching.is_some());
    let watching = state.current_watching.as_ref().unwrap();
    assert_eq!(watching.title, "Test Movie");
    assert_eq!(watching.details, "Action");
    assert_eq!(watching.progress, "45.00%");
}

#[test]
fn test_clear_watching() {
    let mut state = AppState::default();

    // First set something
    state.set_watching(
        "Test Movie".to_string(),
        "Action".to_string(),
        "45.00%".to_string(),
    );
    assert!(state.current_watching.is_some());

    // Then clear it
    state.clear_watching();
    assert!(state.current_watching.is_none());
}

#[test]
fn test_set_discord_connected() {
    let mut state = AppState::default();

    assert!(!state.discord_connected);

    state.set_discord_connected(true);
    assert!(state.discord_connected);

    state.set_discord_connected(false);
    assert!(!state.discord_connected);
}

#[test]
fn test_set_paused() {
    let mut state = AppState::default();

    assert!(!state.is_paused);

    state.set_paused(true);
    assert!(state.is_paused);

    state.set_paused(false);
    assert!(!state.is_paused);
}

#[test]
fn test_status_text_paused() {
    let mut state = AppState::default();
    state.is_paused = true;
    state.discord_connected = true;
    state.current_watching = Some(WatchingInfo {
        title: "Movie".to_string(),
        details: "Details".to_string(),
        progress: "50%".to_string(),
    });

    // Paused takes priority over everything
    assert_eq!(state.status_text(), "Paused");
}

#[test]
fn test_status_text_disconnected() {
    let mut state = AppState::default();
    state.is_paused = false;
    state.discord_connected = false;
    state.current_watching = Some(WatchingInfo {
        title: "Movie".to_string(),
        details: "Details".to_string(),
        progress: "50%".to_string(),
    });

    // Disconnected takes priority over watching
    assert_eq!(state.status_text(), "Connecting to Discord...");
}

#[test]
fn test_status_text_nothing_playing() {
    let mut state = AppState::default();
    state.is_paused = false;
    state.discord_connected = true;
    state.current_watching = None;

    assert_eq!(state.status_text(), "Nothing playing");
}

#[test]
fn test_status_text_watching() {
    let mut state = AppState::default();
    state.discord_connected = true;
    state.set_watching(
        "Inception (2010)".to_string(),
        "Movie".to_string(),
        "45.50%".to_string(),
    );

    assert_eq!(state.status_text(), "Inception (2010) - Movie");
}

#[test]
fn test_watching_info_clone() {
    let info = WatchingInfo {
        title: "Test".to_string(),
        details: "Details".to_string(),
        progress: "50%".to_string(),
    };

    let cloned = info.clone();
    assert_eq!(cloned.title, info.title);
    assert_eq!(cloned.details, info.details);
    assert_eq!(cloned.progress, info.progress);
}
