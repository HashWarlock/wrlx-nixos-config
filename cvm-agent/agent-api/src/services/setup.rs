//! First-run setup service for the CVM Agent.
//!
//! Manages user preferences and setup state, including:
//! - Setup completion detection
//! - User preference storage and retrieval
//! - Configuration wizard support

use std::fs;
use std::path::PathBuf;
use tonic::{Request, Response, Status};

use crate::config::Config;
use super::health::proto::setup_service_server::SetupService;
use super::health::proto::{
    CompleteSetupRequest, CompleteSetupResponse, GetPreferencesRequest,
    ResetSetupRequest, ResetSetupResponse, SavePreferencesResponse,
    SetupStatusRequest, SetupStatusResponse, UserPreferences,
};

const SETUP_VERSION: &str = "1.0";
const SETUP_COMPLETE_FILE: &str = "setup-complete";
const PREFERENCES_FILE: &str = "preferences.json";

/// Serializable preferences for file storage
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct StoredPreferences {
    interaction_mode: i32,
    theme: i32,
    usage_profile: i32,
    memory_preference: i32,
    sensitive_data: i32,
    custom_settings_json: String,
    setup_version: String,
    setup_completed_at_ms: i64,
}

pub struct SetupServiceImpl {
    config_dir: PathBuf,
}

impl SetupServiceImpl {
    pub fn new() -> Self {
        let config = Config::get();
        Self {
            config_dir: config.paths.config_dir.clone(),
        }
    }

    /// Ensure config directory exists
    fn ensure_config_dir(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(&self.config_dir)
    }

    /// Get path to setup-complete marker file
    fn setup_complete_path(&self) -> PathBuf {
        self.config_dir.join(SETUP_COMPLETE_FILE)
    }

    /// Get path to preferences file
    fn preferences_path(&self) -> PathBuf {
        self.config_dir.join(PREFERENCES_FILE)
    }

    /// Check if setup has been completed
    fn is_setup_complete(&self) -> bool {
        self.setup_complete_path().exists()
    }

    /// Load stored preferences from file
    fn load_preferences(&self) -> StoredPreferences {
        let path = self.preferences_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => StoredPreferences::default(),
            }
        } else {
            StoredPreferences::default()
        }
    }

    /// Save preferences to file
    fn save_prefs_to_file(&self, prefs: &StoredPreferences) -> Result<(), std::io::Error> {
        self.ensure_config_dir()?;
        let content = serde_json::to_string_pretty(prefs)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(self.preferences_path(), content)
    }

    /// Convert stored preferences to proto
    fn stored_to_proto(&self, stored: &StoredPreferences) -> UserPreferences {
        UserPreferences {
            interaction_mode: stored.interaction_mode,
            theme: stored.theme,
            usage_profile: stored.usage_profile,
            memory_preference: stored.memory_preference,
            sensitive_data: stored.sensitive_data,
            custom_settings_json: stored.custom_settings_json.clone(),
        }
    }

    /// Convert proto preferences to stored format
    fn proto_to_stored(&self, proto: &UserPreferences, existing: &StoredPreferences) -> StoredPreferences {
        StoredPreferences {
            interaction_mode: proto.interaction_mode,
            theme: proto.theme,
            usage_profile: proto.usage_profile,
            memory_preference: proto.memory_preference,
            sensitive_data: proto.sensitive_data,
            custom_settings_json: proto.custom_settings_json.clone(),
            // Preserve setup metadata
            setup_version: existing.setup_version.clone(),
            setup_completed_at_ms: existing.setup_completed_at_ms,
        }
    }
}

impl Default for SetupServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl SetupService for SetupServiceImpl {
    async fn get_status(
        &self,
        _request: Request<SetupStatusRequest>,
    ) -> Result<Response<SetupStatusResponse>, Status> {
        let setup_complete = self.is_setup_complete();
        let stored = self.load_preferences();

        Ok(Response::new(SetupStatusResponse {
            setup_complete,
            setup_version: if setup_complete {
                stored.setup_version
            } else {
                String::new()
            },
            setup_completed_at_ms: stored.setup_completed_at_ms,
        }))
    }

    async fn get_preferences(
        &self,
        _request: Request<GetPreferencesRequest>,
    ) -> Result<Response<UserPreferences>, Status> {
        let stored = self.load_preferences();

        // Return defaults if no preferences saved
        let prefs = if stored.interaction_mode == 0
            && stored.theme == 0
            && stored.usage_profile == 0
        {
            // Return sensible defaults (using proto enum values directly)
            // INTERACTION_TEXT = 1, THEME_DARK = 1, PROFILE_DEVELOPER = 1
            // MEMORY_REMEMBER_ALL = 1, SENSITIVE_ASK = 2
            UserPreferences {
                interaction_mode: 1, // INTERACTION_TEXT
                theme: 1,            // THEME_DARK
                usage_profile: 1,    // PROFILE_DEVELOPER
                memory_preference: 1, // MEMORY_REMEMBER_ALL
                sensitive_data: 2,   // SENSITIVE_ASK
                custom_settings_json: "{}".to_string(),
            }
        } else {
            self.stored_to_proto(&stored)
        };

        Ok(Response::new(prefs))
    }

    async fn save_preferences(
        &self,
        request: Request<UserPreferences>,
    ) -> Result<Response<SavePreferencesResponse>, Status> {
        let proto = request.into_inner();
        let existing = self.load_preferences();
        let stored = self.proto_to_stored(&proto, &existing);

        self.save_prefs_to_file(&stored)
            .map_err(|e| Status::internal(format!("Failed to save preferences: {}", e)))?;

        tracing::info!("Saved user preferences");

        Ok(Response::new(SavePreferencesResponse { success: true }))
    }

    async fn complete_setup(
        &self,
        request: Request<CompleteSetupRequest>,
    ) -> Result<Response<CompleteSetupResponse>, Status> {
        let req = request.into_inner();
        let version = if req.setup_version.is_empty() {
            SETUP_VERSION.to_string()
        } else {
            req.setup_version
        };

        // Ensure config dir exists
        self.ensure_config_dir()
            .map_err(|e| Status::internal(format!("Failed to create config dir: {}", e)))?;

        // Update preferences with setup metadata
        let mut stored = self.load_preferences();
        stored.setup_version = version.clone();
        stored.setup_completed_at_ms = chrono::Utc::now().timestamp_millis();

        self.save_prefs_to_file(&stored)
            .map_err(|e| Status::internal(format!("Failed to save preferences: {}", e)))?;

        // Create setup-complete marker file
        fs::write(self.setup_complete_path(), &version)
            .map_err(|e| Status::internal(format!("Failed to write setup marker: {}", e)))?;

        tracing::info!("Setup completed (version {})", version);

        Ok(Response::new(CompleteSetupResponse {
            success: true,
            message: format!("Setup complete! Welcome to CVM Agent v{}", version),
        }))
    }

    async fn reset_setup(
        &self,
        _request: Request<ResetSetupRequest>,
    ) -> Result<Response<ResetSetupResponse>, Status> {
        // Remove setup marker
        let marker_path = self.setup_complete_path();
        if marker_path.exists() {
            fs::remove_file(&marker_path)
                .map_err(|e| Status::internal(format!("Failed to remove setup marker: {}", e)))?;
        }

        // Remove preferences file
        let prefs_path = self.preferences_path();
        if prefs_path.exists() {
            fs::remove_file(&prefs_path)
                .map_err(|e| Status::internal(format!("Failed to remove preferences: {}", e)))?;
        }

        tracing::info!("Setup reset");

        Ok(Response::new(ResetSetupResponse { success: true }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_service() -> (SetupServiceImpl, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let service = SetupServiceImpl {
            config_dir: temp_dir.path().to_path_buf(),
        };
        (service, temp_dir)
    }

    #[tokio::test]
    async fn test_setup_not_complete_initially() {
        let (service, _temp) = create_test_service();

        let response = service
            .get_status(Request::new(SetupStatusRequest {}))
            .await
            .unwrap();

        assert!(!response.into_inner().setup_complete);
    }

    #[tokio::test]
    async fn test_complete_setup() {
        let (service, _temp) = create_test_service();

        // Complete setup
        let response = service
            .complete_setup(Request::new(CompleteSetupRequest {
                setup_version: "1.0".to_string(),
            }))
            .await
            .unwrap();

        assert!(response.into_inner().success);

        // Verify status
        let status = service
            .get_status(Request::new(SetupStatusRequest {}))
            .await
            .unwrap();

        let status_inner = status.into_inner();
        assert!(status_inner.setup_complete);
        assert_eq!(status_inner.setup_version, "1.0");
    }

    #[tokio::test]
    async fn test_save_and_load_preferences() {
        let (service, _temp) = create_test_service();

        // Using proto enum integer values directly
        // INTERACTION_VOICE = 2, THEME_LIGHT = 2, PROFILE_CREATIVE = 2
        // MEMORY_ASK_FIRST = 3, SENSITIVE_ENCRYPT = 1
        let prefs = UserPreferences {
            interaction_mode: 2, // INTERACTION_VOICE
            theme: 2,            // THEME_LIGHT
            usage_profile: 2,    // PROFILE_CREATIVE
            memory_preference: 3, // MEMORY_ASK_FIRST
            sensitive_data: 1,   // SENSITIVE_ENCRYPT
            custom_settings_json: r#"{"foo":"bar"}"#.to_string(),
        };

        // Save preferences
        let save_response = service
            .save_preferences(Request::new(prefs.clone()))
            .await
            .unwrap();

        assert!(save_response.into_inner().success);

        // Load preferences
        let loaded = service
            .get_preferences(Request::new(GetPreferencesRequest {}))
            .await
            .unwrap();

        let loaded_inner = loaded.into_inner();
        assert_eq!(loaded_inner.interaction_mode, 2); // INTERACTION_VOICE
        assert_eq!(loaded_inner.theme, 2);            // THEME_LIGHT
        assert_eq!(loaded_inner.usage_profile, 2);    // PROFILE_CREATIVE
    }

    #[tokio::test]
    async fn test_reset_setup() {
        let (service, _temp) = create_test_service();

        // Complete setup first
        service
            .complete_setup(Request::new(CompleteSetupRequest {
                setup_version: "1.0".to_string(),
            }))
            .await
            .unwrap();

        // Save some preferences
        service
            .save_preferences(Request::new(UserPreferences {
                interaction_mode: 2, // INTERACTION_VOICE
                theme: 1,            // THEME_DARK
                usage_profile: 1,    // PROFILE_DEVELOPER
                memory_preference: 1, // MEMORY_REMEMBER_ALL
                sensitive_data: 3,   // SENSITIVE_SKIP
                custom_settings_json: "{}".to_string(),
            }))
            .await
            .unwrap();

        // Reset
        let reset_response = service
            .reset_setup(Request::new(ResetSetupRequest {}))
            .await
            .unwrap();

        assert!(reset_response.into_inner().success);

        // Verify setup is incomplete
        let status = service
            .get_status(Request::new(SetupStatusRequest {}))
            .await
            .unwrap();

        assert!(!status.into_inner().setup_complete);
    }

    #[tokio::test]
    async fn test_default_preferences() {
        let (service, _temp) = create_test_service();

        // Get preferences without saving any
        let prefs = service
            .get_preferences(Request::new(GetPreferencesRequest {}))
            .await
            .unwrap();

        let prefs_inner = prefs.into_inner();
        // Should return defaults
        assert_eq!(prefs_inner.interaction_mode, 1); // INTERACTION_TEXT
        assert_eq!(prefs_inner.theme, 1);            // THEME_DARK
        assert_eq!(prefs_inner.usage_profile, 1);    // PROFILE_DEVELOPER
    }
}
