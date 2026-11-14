//! Persistence keys and analytics events.
//!
//! The Dart layer stores update state and suppresses UI re-prompts under these
//! keys (all present as literals in `rode_bridge.dll` `.rdata`, and mirrored by
//! the `package:rode_central/firmware_update` use cases):
//!
//! ```text
//! firmware_update_required
//! firmware_update_started
//! firmware_update_progress
//! firmware_update_completed
//! dismissedFirmwareUpdateTile
//! hide_apply_changes_dialog
//! hide_formatting_memory_dialog
//! ```

use crate::json::Json;
use crate::state::FirmwareUpdateStatus;

/// Keys used by the update feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PersistenceKey {
    /// Set when the diagnostics check decided an update is available.
    UpdateRequired,
    /// Set when an update starts.
    UpdateStarted,
    /// Last whole-percent progress.
    UpdateProgress,
    /// Set when an update finishes.
    UpdateCompleted,
    /// The user dismissed the update tile for a device.
    DismissedUpdateTile,
    /// The user asked not to see the apply-changes dialog again.
    HideApplyChangesDialog,
    /// The user asked not to see the format-memory dialog again.
    HideFormattingMemoryDialog,
}

impl PersistenceKey {
    /// Every key.
    pub const ALL: [PersistenceKey; 7] = [
        PersistenceKey::UpdateRequired,
        PersistenceKey::UpdateStarted,
        PersistenceKey::UpdateProgress,
        PersistenceKey::UpdateCompleted,
        PersistenceKey::DismissedUpdateTile,
        PersistenceKey::HideApplyChangesDialog,
        PersistenceKey::HideFormattingMemoryDialog,
    ];

    /// The literal stored on disk.
    pub fn as_str(self) -> &'static str {
        match self {
            PersistenceKey::UpdateRequired => "firmware_update_required",
            PersistenceKey::UpdateStarted => "firmware_update_started",
            PersistenceKey::UpdateProgress => "firmware_update_progress",
            PersistenceKey::UpdateCompleted => "firmware_update_completed",
            PersistenceKey::DismissedUpdateTile => "dismissedFirmwareUpdateTile",
            PersistenceKey::HideApplyChangesDialog => "hide_apply_changes_dialog",
            PersistenceKey::HideFormattingMemoryDialog => "hide_formatting_memory_dialog",
        }
    }

    /// Inverse lookup.
    pub fn parse(key: &str) -> Option<PersistenceKey> {
        PersistenceKey::ALL.into_iter().find(|k| k.as_str() == key)
    }
}

/// Analytics events emitted around an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnalyticsEvent {
    /// `firmware_update_required`
    Required,
    /// `firmware_update_started`
    Started,
    /// `firmware_update_progress`
    Progress,
    /// `firmware_update_completed`
    Completed,
}

impl AnalyticsEvent {
    /// The analytics event name (identical to the persistence key).
    pub fn as_str(self) -> &'static str {
        match self {
            AnalyticsEvent::Required => "firmware_update_required",
            AnalyticsEvent::Started => "firmware_update_started",
            AnalyticsEvent::Progress => "firmware_update_progress",
            AnalyticsEvent::Completed => "firmware_update_completed",
        }
    }
}

/// A snapshot of persisted update state for one device.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedUpdateState {
    /// Stable device identifier (serial number or HID path).
    pub device_id: String,
    /// Whether an update was found to be required.
    pub required: bool,
    /// Whether an update is currently running.
    pub started: bool,
    /// Last reported progress, 0..=100.
    pub progress: u8,
    /// Whether the last update completed.
    pub completed: bool,
    /// Last status reported to the UI.
    pub status: FirmwareUpdateStatus,
}

impl PersistedUpdateState {
    /// Fresh state for a device.
    pub fn new(device_id: impl Into<String>) -> Self {
        PersistedUpdateState {
            device_id: device_id.into(),
            required: false,
            started: false,
            progress: 0,
            completed: false,
            status: FirmwareUpdateStatus::Idle,
        }
    }

    /// Key/value pairs the Dart layer would write.
    pub fn entries(&self) -> Vec<(PersistenceKey, Json)> {
        vec![
            (PersistenceKey::UpdateRequired, Json::Bool(self.required)),
            (PersistenceKey::UpdateStarted, Json::Bool(self.started)),
            (
                PersistenceKey::UpdateProgress,
                Json::Number(self.progress as f64),
            ),
            (PersistenceKey::UpdateCompleted, Json::Bool(self.completed)),
        ]
    }

    /// Compact JSON object keyed by the literal persistence keys.
    pub fn to_json(&self) -> String {
        let mut entries: Vec<(String, Json)> = self
            .entries()
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect();
        entries.push((
            "device_id".to_string(),
            Json::String(self.device_id.clone()),
        ));
        entries.push((
            "status".to_string(),
            Json::String(self.status.as_str().to_string()),
        ));
        Json::Object(entries).to_compact_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        for key in PersistenceKey::ALL {
            assert_eq!(PersistenceKey::parse(key.as_str()), Some(key));
        }
    }

    #[test]
    fn keys_are_the_observed_literals() {
        assert_eq!(
            PersistenceKey::UpdateRequired.as_str(),
            "firmware_update_required"
        );
        assert_eq!(
            PersistenceKey::DismissedUpdateTile.as_str(),
            "dismissedFirmwareUpdateTile"
        );
        assert_eq!(
            PersistenceKey::HideApplyChangesDialog.as_str(),
            "hide_apply_changes_dialog"
        );
    }

    #[test]
    fn state_serialises() {
        let mut s = PersistedUpdateState::new("SN123");
        s.required = true;
        s.started = true;
        s.progress = 42;
        let json = s.to_json();
        let parsed = Json::parse(&json).unwrap();
        assert_eq!(
            parsed
                .get("firmware_update_progress")
                .and_then(Json::as_u64),
            Some(42)
        );
        assert_eq!(
            parsed.get("device_id").and_then(Json::as_str),
            Some("SN123")
        );
    }
}
