// Tests for setup module in src/setup/
// Note: Most of the setup server internals are private. The main public interface
// is run_setup_server() which starts an HTTP server - difficult to test in isolation.
// The inline tests in server.rs cover the private types.
// These tests cover the public SetupResult type and any observable behaviors.

use discrakt::setup::SetupResult;

// ============================================================================
// SetupResult Tests
// ============================================================================

#[test]
fn test_setup_result_fields() {
    let result = SetupResult {
        trakt_username: "testuser".to_string(),
        trakt_client_id: "client123".to_string(),
    };

    assert_eq!(result.trakt_username, "testuser");
    assert_eq!(result.trakt_client_id, "client123");
}

#[test]
fn test_setup_result_empty_client_id() {
    // Empty client ID is valid - will use default
    let result = SetupResult {
        trakt_username: "testuser".to_string(),
        trakt_client_id: "".to_string(),
    };

    assert_eq!(result.trakt_username, "testuser");
    assert!(result.trakt_client_id.is_empty());
}

#[test]
fn test_setup_result_with_special_characters() {
    let result = SetupResult {
        trakt_username: "test_user-123".to_string(),
        trakt_client_id: "abc123def456ghi789".to_string(),
    };

    assert_eq!(result.trakt_username, "test_user-123");
    assert_eq!(result.trakt_client_id, "abc123def456ghi789");
}

// Note: Additional tests for the setup server internals are in src/setup/server.rs
// as inline #[cfg(test)] tests, since they need access to private types like:
// - SubmittedCredentials
// - StatusResponse
// - DeviceCodeResponse
// - OAuthState
