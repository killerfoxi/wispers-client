//! File-based storage for node state.

use crate::storage::NodeStateStore;
use crate::types::{AppNamespace, NodeRegistration, NodeState, ProfileNamespace, RootKey, ROOT_KEY_LEN};
use std::fs;
use std::io;
use std::path::PathBuf;

/// File-based node state store.
///
/// Stores state in a directory structure:
/// ```text
/// base_dir/
///   {app_namespace}/
///     {profile_namespace}/
///       root_key.bin
///       registration.json
/// ```
pub struct FileNodeStateStore {
    base_dir: PathBuf,
}

#[derive(Debug)]
pub enum FileStoreError {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidRootKey,
}

impl std::fmt::Display for FileStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileStoreError::Io(e) => write!(f, "I/O error: {e}"),
            FileStoreError::Json(e) => write!(f, "JSON error: {e}"),
            FileStoreError::InvalidRootKey => write!(f, "invalid root key length"),
        }
    }
}

impl std::error::Error for FileStoreError {}

impl From<io::Error> for FileStoreError {
    fn from(e: io::Error) -> Self {
        FileStoreError::Io(e)
    }
}

impl From<serde_json::Error> for FileStoreError {
    fn from(e: serde_json::Error) -> Self {
        FileStoreError::Json(e)
    }
}

impl FileNodeStateStore {
    /// Create a new file-based store with the given base directory.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Create a store using the default config directory.
    ///
    /// On Linux: `~/.config/{app_name}/`
    /// On macOS: `~/Library/Application Support/{app_name}/`
    /// On Windows: `%APPDATA%/{app_name}/`
    pub fn with_app_name(app_name: &str) -> Option<Self> {
        let config_dir = dirs::config_dir()?;
        Some(Self::new(config_dir.join(app_name)))
    }

    fn state_dir(&self, app: &AppNamespace, profile: &ProfileNamespace) -> PathBuf {
        self.base_dir.join(app.as_ref()).join(profile.as_ref())
    }

    fn root_key_path(&self, app: &AppNamespace, profile: &ProfileNamespace) -> PathBuf {
        self.state_dir(app, profile).join("root_key.bin")
    }

    fn registration_path(&self, app: &AppNamespace, profile: &ProfileNamespace) -> PathBuf {
        self.state_dir(app, profile).join("registration.json")
    }
}

impl NodeStateStore for FileNodeStateStore {
    type Error = FileStoreError;

    fn load(
        &self,
        app_namespace: &AppNamespace,
        profile_namespace: &ProfileNamespace,
    ) -> Result<Option<NodeState>, Self::Error> {
        let root_key_path = self.root_key_path(app_namespace, profile_namespace);

        // If root key doesn't exist, state doesn't exist
        if !root_key_path.exists() {
            return Ok(None);
        }

        // Load root key
        let root_key_bytes = fs::read(&root_key_path)?;
        if root_key_bytes.len() != ROOT_KEY_LEN {
            return Err(FileStoreError::InvalidRootKey);
        }
        let mut key_array = [0u8; ROOT_KEY_LEN];
        key_array.copy_from_slice(&root_key_bytes);

        // Load registration if present
        let registration_path = self.registration_path(app_namespace, profile_namespace);
        let registration = if registration_path.exists() {
            let json = fs::read_to_string(&registration_path)?;
            Some(serde_json::from_str::<NodeRegistration>(&json)?)
        } else {
            None
        };

        let mut state = NodeState::initialize_with_namespaces(
            app_namespace.clone(),
            profile_namespace.clone(),
        );
        state.root_key = RootKey::from_bytes(key_array);
        state.registration = registration;
        Ok(Some(state))
    }

    fn save(&self, state: &NodeState) -> Result<(), Self::Error> {
        let dir = self.state_dir(&state.app_namespace, &state.profile_namespace);
        fs::create_dir_all(&dir)?;

        // Save root key
        let root_key_path = self.root_key_path(&state.app_namespace, &state.profile_namespace);
        fs::write(&root_key_path, state.root_key.as_bytes())?;

        // Save registration if present
        let registration_path = self.registration_path(&state.app_namespace, &state.profile_namespace);
        if let Some(ref registration) = state.registration {
            let json = serde_json::to_string_pretty(registration)?;
            fs::write(&registration_path, json)?;
        } else if registration_path.exists() {
            fs::remove_file(&registration_path)?;
        }

        Ok(())
    }

    fn delete(
        &self,
        app_namespace: &AppNamespace,
        profile_namespace: &ProfileNamespace,
    ) -> Result<(), Self::Error> {
        let dir = self.state_dir(app_namespace, profile_namespace);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}
