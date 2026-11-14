//! Update states, phases and progress.
//!
//! Two layers are modelled separately:
//!
//! * [`FirmwareUpdateStatus`] — the six values the Dart layer exposes
//!   (`rode_bridge/firmware_update/firmware_update_status.dart`). These are
//!   observed, not guessed.
//! * [`Phase`] — the finer-grained internal progression reconstructed from the
//!   updater's responsibilities (download → verify → prepare bootloader →
//!   erase → write → validate → apply → reconnect). The C++ `State` enum's
//!   numeric values are not present in the stripped binary, so this enum is a
//!   reconstruction and its discriminants carry no meaning.
//!
//! [`FlashMode`] corresponds to `FirmwareUpdater::W4FlashMode`, which the base
//! class also publishes as a signal parameter. Its enumerators are not
//! recoverable either; the three modes below are the reconstruction.

use std::fmt;

/// The status values the Dart bridge exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FirmwareUpdateStatus {
    /// No update in progress.
    Idle,
    /// Firmware package being fetched.
    Downloading,
    /// Package being verified and written to the device.
    Installing,
    /// Device applying the new image (typically across a reboot).
    Applying,
    /// Transfer finished successfully.
    Complete,
    /// Device re-enumerated after the update.
    DeviceConnected,
}

impl FirmwareUpdateStatus {
    /// Every status in progression order.
    pub const ALL: [FirmwareUpdateStatus; 6] = [
        FirmwareUpdateStatus::Idle,
        FirmwareUpdateStatus::Downloading,
        FirmwareUpdateStatus::Installing,
        FirmwareUpdateStatus::Applying,
        FirmwareUpdateStatus::Complete,
        FirmwareUpdateStatus::DeviceConnected,
    ];

    /// The Dart enum name, as used by the bridge payloads.
    pub fn as_str(self) -> &'static str {
        match self {
            FirmwareUpdateStatus::Idle => "Idle",
            FirmwareUpdateStatus::Downloading => "Downloading",
            FirmwareUpdateStatus::Installing => "Installing",
            FirmwareUpdateStatus::Applying => "Applying",
            FirmwareUpdateStatus::Complete => "Complete",
            FirmwareUpdateStatus::DeviceConnected => "DeviceConnected",
        }
    }

    /// Inverse of [`FirmwareUpdateStatus::as_str`].
    pub fn parse(text: &str) -> Option<FirmwareUpdateStatus> {
        FirmwareUpdateStatus::ALL
            .into_iter()
            .find(|s| s.as_str() == text.trim())
    }
}

impl fmt::Display for FirmwareUpdateStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Internal progression of an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Nothing running.
    Idle,
    /// Comparing the device version against the manifest.
    Checking,
    /// Fetching the package or reading a local binary.
    Downloading,
    /// Checking size, CRC-32 and optional MD5.
    Verifying,
    /// Handshaking with the device / entering bootloader.
    Preparing,
    /// Erasing the target partition(s).
    Erasing,
    /// Writing payload bytes.
    Writing,
    /// Reading back and comparing the checksum.
    Validating,
    /// Rebooting the device into the new image.
    Applying,
    /// Waiting for the device to re-enumerate.
    Reconnecting,
    /// Finished successfully.
    Done,
    /// Aborted with an error.
    Failed,
    /// Aborted by the user.
    Cancelled,
}

impl Phase {
    /// Every phase, in pipeline order (terminal failure/cancel last).
    pub const ALL: [Phase; 13] = [
        Phase::Idle,
        Phase::Checking,
        Phase::Downloading,
        Phase::Verifying,
        Phase::Preparing,
        Phase::Erasing,
        Phase::Writing,
        Phase::Validating,
        Phase::Applying,
        Phase::Reconnecting,
        Phase::Done,
        Phase::Failed,
        Phase::Cancelled,
    ];

    /// The status this phase is reported under.
    pub fn status(self) -> FirmwareUpdateStatus {
        match self {
            Phase::Idle | Phase::Checking => FirmwareUpdateStatus::Idle,
            Phase::Downloading => FirmwareUpdateStatus::Downloading,
            Phase::Verifying
            | Phase::Preparing
            | Phase::Erasing
            | Phase::Writing
            | Phase::Validating => FirmwareUpdateStatus::Installing,
            Phase::Applying | Phase::Reconnecting => FirmwareUpdateStatus::Applying,
            Phase::Done => FirmwareUpdateStatus::Complete,
            Phase::Failed | Phase::Cancelled => FirmwareUpdateStatus::Idle,
        }
    }

    /// True when no further transition is possible except a reset.
    pub fn is_terminal(self) -> bool {
        matches!(self, Phase::Done | Phase::Failed | Phase::Cancelled)
    }

    /// Ordinal used for the "moves forward" check. `Failed`/`Cancelled` are
    /// reachable from anywhere and are not part of the ordinal sequence.
    fn forward_rank(self) -> Option<u8> {
        Some(match self {
            Phase::Idle => 0,
            Phase::Checking => 1,
            Phase::Downloading => 2,
            Phase::Verifying => 3,
            Phase::Preparing => 4,
            Phase::Erasing => 5,
            Phase::Writing => 6,
            Phase::Validating => 7,
            Phase::Applying => 8,
            Phase::Reconnecting => 9,
            Phase::Done => 10,
            Phase::Failed | Phase::Cancelled => return None,
        })
    }

    /// The name shown by the CLI.
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Checking => "checking",
            Phase::Downloading => "downloading",
            Phase::Verifying => "verifying",
            Phase::Preparing => "preparing",
            Phase::Erasing => "erasing",
            Phase::Writing => "writing",
            Phase::Validating => "validating",
            Phase::Applying => "applying",
            Phase::Reconnecting => "reconnecting",
            Phase::Done => "done",
            Phase::Failed => "failed",
            Phase::Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `FirmwareUpdater::W4FlashMode` — how the device is put into flash mode.
///
/// The enumerators are not recoverable from the stripped binary; only the
/// signal parameter type is. The names below are a reconstruction matching
/// the three behaviours the use cases expose (normal update, forced update
/// from `forceFirmwareUpdate`, and recovery of a half-flashed device).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlashMode {
    /// Standard update path.
    Normal,
    /// Update requested with `forceFirmwareUpdate`, ignoring the version gate.
    Forced,
    /// Device left in bootloader by an interrupted update.
    Recovery,
}

impl FlashMode {
    /// All modes.
    pub const ALL: [FlashMode; 3] = [FlashMode::Normal, FlashMode::Forced, FlashMode::Recovery];

    /// Lower-case label.
    pub fn as_str(self) -> &'static str {
        match self {
            FlashMode::Normal => "normal",
            FlashMode::Forced => "forced",
            FlashMode::Recovery => "recovery",
        }
    }
}

impl fmt::Display for FlashMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A transition the state machine refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IllegalTransition {
    /// Current phase.
    pub from: Phase,
    /// Requested phase.
    pub to: Phase,
}

impl fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot move from {} to {}", self.from, self.to)
    }
}

impl std::error::Error for IllegalTransition {}

/// A progress snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// Dart-facing status.
    pub status: FirmwareUpdateStatus,
    /// Internal phase.
    pub phase: Phase,
    /// Completion of the whole update, 0..=100.
    pub percent: u8,
    /// Partition currently being written, when applicable.
    pub partition: Option<String>,
    /// Fractional progress in `0.0..=1.0`, matching the manager's
    /// `void(double)` signal.
    pub fraction: f64,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            status: FirmwareUpdateStatus::Idle,
            phase: Phase::Idle,
            percent: 0,
            partition: None,
            fraction: 0.0,
        }
    }
}

impl Progress {
    /// Progress at the start of an update.
    pub fn start() -> Self {
        Self::default()
    }
}

/// Monotonic phase/progress tracker.
#[derive(Debug, Clone)]
pub struct UpdateStateMachine {
    phase: Phase,
    percent: u8,
    partition: Option<String>,
}

impl Default for UpdateStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateStateMachine {
    /// A machine in [`Phase::Idle`] at 0 %.
    pub const fn new() -> Self {
        UpdateStateMachine {
            phase: Phase::Idle,
            percent: 0,
            partition: None,
        }
    }

    /// Current phase.
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Current whole-percent progress.
    pub const fn percent(&self) -> u8 {
        self.percent
    }

    /// Moves to `to`, rejecting backward or skipped-to-terminal jumps.
    pub fn transition(&mut self, to: Phase) -> Result<(), IllegalTransition> {
        if to == self.phase {
            return Ok(());
        }
        // Failure and cancellation are reachable from any non-terminal phase.
        if matches!(to, Phase::Failed | Phase::Cancelled) {
            if self.phase.is_terminal() {
                return Err(IllegalTransition {
                    from: self.phase,
                    to,
                });
            }
            self.phase = to;
            return Ok(());
        }
        if self.phase.is_terminal() {
            return Err(IllegalTransition {
                from: self.phase,
                to,
            });
        }
        let (Some(from), Some(target)) = (self.phase.forward_rank(), to.forward_rank()) else {
            return Err(IllegalTransition {
                from: self.phase,
                to,
            });
        };
        // Forward only. Skipping ahead is allowed (a fast device may jump from
        // `Erasing` straight to `Validating`), but never moving backwards.
        if target <= from {
            return Err(IllegalTransition {
                from: self.phase,
                to,
            });
        }
        self.phase = to;
        Ok(())
    }

    /// Sets the partition label for subsequent progress reports.
    pub fn set_partition(&mut self, name: Option<String>) {
        self.partition = name;
    }

    /// Records absolute progress, clamped to `0..=100` and never decreasing.
    pub fn set_percent(&mut self, percent: u8) {
        self.percent = self.percent.max(percent.min(100));
    }

    /// Adds `delta` percent.
    pub fn bump(&mut self, delta: u8) {
        self.set_percent(self.percent.saturating_add(delta));
    }

    /// Snapshot for emission.
    pub fn snapshot(&self) -> Progress {
        Progress {
            status: self.phase.status(),
            phase: self.phase,
            percent: self.percent,
            partition: self.partition.clone(),
            fraction: self.percent as f64 / 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_names_match_dart() {
        for s in FirmwareUpdateStatus::ALL {
            assert_eq!(FirmwareUpdateStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(FirmwareUpdateStatus::parse("nope"), None);
    }

    #[test]
    fn phase_status_mapping() {
        assert_eq!(Phase::Verifying.status(), FirmwareUpdateStatus::Installing);
        assert_eq!(Phase::Writing.status(), FirmwareUpdateStatus::Installing);
        assert_eq!(Phase::Applying.status(), FirmwareUpdateStatus::Applying);
        assert_eq!(Phase::Done.status(), FirmwareUpdateStatus::Complete);
        assert_eq!(Phase::Failed.status(), FirmwareUpdateStatus::Idle);
    }

    #[test]
    fn state_machine_moves_forward_only() {
        let mut m = UpdateStateMachine::new();
        m.transition(Phase::Checking).unwrap();
        m.transition(Phase::Downloading).unwrap();
        assert!(m.transition(Phase::Checking).is_err());
        m.transition(Phase::Erasing).unwrap(); // skipping ahead is allowed
        m.transition(Phase::Done).unwrap();
        assert!(m.transition(Phase::Writing).is_err());
    }

    #[test]
    fn failure_is_reachable_from_any_live_phase() {
        let mut m = UpdateStateMachine::new();
        m.transition(Phase::Writing).unwrap();
        m.transition(Phase::Failed).unwrap();
        assert!(m.transition(Phase::Downloading).is_err());
    }

    #[test]
    fn percent_is_monotonic_and_clamped() {
        let mut m = UpdateStateMachine::new();
        m.set_percent(40);
        m.set_percent(10);
        assert_eq!(m.percent(), 40);
        m.set_percent(250);
        assert_eq!(m.percent(), 100);
    }

    #[test]
    fn snapshot_converts_to_fraction() {
        let mut m = UpdateStateMachine::new();
        m.transition(Phase::Writing).unwrap();
        m.set_percent(50);
        m.set_partition(Some("fw0_sys".into()));
        let p = m.snapshot();
        assert_eq!(p.status, FirmwareUpdateStatus::Installing);
        assert_eq!(p.fraction, 0.5);
        assert_eq!(p.partition.as_deref(), Some("fw0_sys"));
    }
}
