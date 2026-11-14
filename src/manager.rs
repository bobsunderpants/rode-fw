//! Update orchestration: firmware planning, payload verification and the
//! flash pipeline.
//!
//! The bridge exposes this as `FirmwareUpdateManager` plus the
//! `FirmwareUpdater` hierarchy. What is observable in the shipped binary is:
//!
//! * the decision inputs (`DiagnosticsCheckFirmwareUseCase`, `checkFirmware`,
//!   `forceFirmwareUpdate`, the `firmware_update_required` flag);
//! * the ordered stages (`Downloading` → `Installing` → `Applying` →
//!   `Complete` → `DeviceConnected` in the Dart layer);
//! * the verification primitives (CRC-32 per partition, MD5 for the package,
//!   `Invalid Nordic firmware.` for nRF payloads);
//! * the progress signals `void(int)` / `void(double)` / `void(bool)`.
//!
//! What is *not* observable is the per-family byte-level protocol: no HID
//! report layouts, command opcodes or bootloader handshakes survive in the
//! stripped DLL. Those live behind [`FirmwareTransport`], which is the single
//! seam a real port must implement; see `docs/GAPS.md`.
//!
//! The pipeline below is therefore a reconstruction of the **ordering and the
//! checks**, deliberately strict about everything the artifacts do prove and
//! explicit about the one thing they do not (the wire format).

use std::collections::BTreeMap;
use std::fmt;

use crate::crc32;
use crate::device::{self, BoardProfile, DeviceFamily, UpdaterKind};
use crate::download::{Downloader, FirmwareManifest};
use crate::error::{Error, ImageError, Result};
use crate::ota_xml::{ManifestError, OtaManifest, Partition, PartitionType};
use crate::persistence::{AnalyticsEvent, PersistedUpdateState};
use crate::state::{FlashMode, Phase, Progress, UpdateStateMachine};
use crate::version::FwVersion;

/// Bytes handed to [`FirmwareTransport::write`] per call.
///
/// The real transports use HID reports or DFU packets whose size is not
/// recoverable; 1 KiB keeps the number of trait calls low without assuming a
/// particular frame size.
pub const WRITE_CHUNK: usize = 1024;

/// Number of milliseconds the manager waits for the device to re-enumerate.
pub const RECONNECT_TIMEOUT_MS: u64 = 30_000;

/// A connected device as the update feature sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Board id reported by the device, matched against `<board_name>`.
    pub board: String,
    /// Firmware version currently running on the device.
    pub version: FwVersion,
    /// Serial number or HID path, used as the persistence key.
    pub serial: Option<String>,
}

impl DeviceInfo {
    /// Builds a device description.
    pub fn new(board: impl Into<String>, version: FwVersion) -> Self {
        DeviceInfo {
            board: board.into(),
            version,
            serial: None,
        }
    }

    /// Builds a device from string forms, parsing the version.
    pub fn from_strs(board: &str, version: &str) -> Result<Self> {
        Ok(DeviceInfo::new(board, FwVersion::from_name(version)?))
    }

    /// Adds a serial number / HID path.
    pub fn with_serial(mut self, serial: impl Into<String>) -> Self {
        self.serial = Some(serial.into());
        self
    }

    /// Product family, when the board is known.
    pub fn family(&self) -> Option<DeviceFamily> {
        device::board_profile(&self.board).map(|p| p.family)
    }

    /// Static board description, when the board appears in the manifests.
    pub fn profile(&self) -> Option<&'static BoardProfile> {
        device::board_profile(&self.board)
    }

    /// Updater class the bridge would instantiate.
    pub fn updater(&self) -> UpdaterKind {
        device::resolve_updater(&self.board)
    }

    /// Persistence key for this device.
    pub fn id(&self) -> String {
        self.serial
            .clone()
            .unwrap_or_else(|| format!("{}@{}", self.board, self.version.code))
    }
}

/// How strongly the user is pushed towards an available update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UpdatePolicy {
    /// The update cannot be skipped (forced update, or below the minimum).
    Required,
    /// A newer version exists; the tile is shown but dismissible.
    Recommended,
    /// Only offered on request (reinstall of the same version).
    Optional,
}

impl UpdatePolicy {
    /// True for [`UpdatePolicy::Required`].
    pub const fn is_required(self) -> bool {
        matches!(self, UpdatePolicy::Required)
    }

    /// Lower-case label used by the CLI and analytics.
    pub const fn as_str(self) -> &'static str {
        match self {
            UpdatePolicy::Required => "required",
            UpdatePolicy::Recommended => "recommended",
            UpdatePolicy::Optional => "optional",
        }
    }
}

impl fmt::Display for UpdatePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a plan was produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateReason {
    /// `forceFirmwareUpdate` was called; the version gate is ignored.
    Forced,
    /// The device is below the minimum version the app accepts.
    BelowMinimum {
        /// The minimum version that was configured.
        minimum: FwVersion,
    },
    /// The manifest advertises a newer version.
    NewerAvailable,
    /// Same version, explicitly requested again.
    Reinstall,
    /// Device and manifest agree.
    UpToDate,
    /// The device is already newer than the manifest.
    NewerOnDevice,
}

impl UpdateReason {
    /// Short label for logs.
    pub fn as_str(&self) -> &'static str {
        match self {
            UpdateReason::Forced => "forced",
            UpdateReason::BelowMinimum { .. } => "below-minimum",
            UpdateReason::NewerAvailable => "newer-available",
            UpdateReason::Reinstall => "reinstall",
            UpdateReason::UpToDate => "up-to-date",
            UpdateReason::NewerOnDevice => "newer-on-device",
        }
    }
}

impl fmt::Display for UpdateReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Inputs that let the caller override the plain "newer version" gate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateGate {
    /// Minimum acceptable version; anything below it is [`UpdatePolicy::Required`].
    pub minimum: Option<FwVersion>,
    /// Mirror of `forceFirmwareUpdate`: always plan an update.
    pub forced: bool,
    /// Permit re-flashing the same version.
    pub allow_reinstall: bool,
}

impl UpdateGate {
    /// Gate that only reacts to newer versions.
    pub fn normal() -> Self {
        UpdateGate::default()
    }

    /// Gate that mirrors `forceFirmwareUpdate`.
    pub fn forced() -> Self {
        UpdateGate {
            forced: true,
            ..UpdateGate::default()
        }
    }

    /// Sets the minimum accepted version.
    pub fn with_minimum(mut self, minimum: FwVersion) -> Self {
        self.minimum = Some(minimum);
        self
    }
}

/// A decision produced by [`check_firmware`].
#[derive(Debug, Clone, PartialEq)]
pub enum PlanDecision {
    /// Nothing to do.
    UpToDate {
        /// Version on the device.
        current: FwVersion,
        /// Version in the manifest.
        latest: FwVersion,
        /// Why no update is offered.
        reason: UpdateReason,
    },
    /// An update should be performed.
    Update(Box<UpdatePlan>),
}

impl PlanDecision {
    /// The plan, when there is one.
    pub fn plan(&self) -> Option<&UpdatePlan> {
        match self {
            PlanDecision::Update(plan) => Some(plan),
            PlanDecision::UpToDate { .. } => None,
        }
    }

    /// True when an update is planned.
    pub const fn is_update(&self) -> bool {
        matches!(self, PlanDecision::Update(_))
    }
}

/// Everything needed to run an update.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdatePlan {
    /// Board the manifest targets.
    pub board: String,
    /// Product family (from the board table).
    pub family: DeviceFamily,
    /// Updater class the bridge would instantiate.
    pub updater: UpdaterKind,
    /// Version currently on the device.
    pub from: FwVersion,
    /// Version the manifest installs.
    pub to: FwVersion,
    /// How hard the update is pushed.
    pub policy: UpdatePolicy,
    /// Why the plan exists.
    pub reason: UpdateReason,
    /// Sum of all partition sizes.
    pub total_bytes: u64,
    /// The manifest that drives the update.
    pub manifest: OtaManifest,
}

impl UpdatePlan {
    /// Flash mode implied by the policy.
    pub fn flash_mode(&self) -> FlashMode {
        match self.policy {
            UpdatePolicy::Required => FlashMode::Forced,
            UpdatePolicy::Recommended | UpdatePolicy::Optional => FlashMode::Normal,
        }
    }

    /// Executes the plan against `transport`, verifying `payloads` first.
    pub fn execute<T, S>(
        &self,
        payloads: &PayloadSet,
        transport: &mut T,
        sink: &mut S,
    ) -> Result<UpdateOutcome>
    where
        T: FirmwareTransport,
        S: EventSink,
    {
        self.execute_inner(payloads, transport, sink, &mut |_| false)
    }

    /// Like [`UpdatePlan::execute`], with an extra cancellation probe checked
    /// between partitions and write chunks.
    pub fn execute_with<T, S, F>(
        &self,
        payloads: &PayloadSet,
        transport: &mut T,
        sink: &mut S,
        cancel: &mut F,
    ) -> Result<UpdateOutcome>
    where
        T: FirmwareTransport,
        S: EventSink,
        F: FnMut(&Progress) -> bool,
    {
        self.execute_inner(payloads, transport, sink, cancel)
    }

    fn execute_inner<T, S, F>(
        &self,
        payloads: &PayloadSet,
        transport: &mut T,
        sink: &mut S,
        cancel: &mut F,
    ) -> Result<UpdateOutcome>
    where
        T: FirmwareTransport,
        S: EventSink,
        F: FnMut(&Progress) -> bool,
    {
        let mut machine = UpdateStateMachine::new();
        let mut last: Option<(u8, Phase)> = None;
        let device_id = payloads
            .device_id
            .clone()
            .unwrap_or_else(|| format!("{}@{}", self.board, self.to.code));

        // Required updates are recorded before anything is downloaded, which is
        // what `firmware_update_required` in the Dart layer tracks.
        if self.policy.is_required() {
            sink.event(UpdateEvent::Analytics(AnalyticsEvent::Required));
        }
        sink.event(UpdateEvent::Analytics(AnalyticsEvent::Started));

        let outcome = (|| -> Result<UpdateOutcome> {
            machine
                .transition(Phase::Verifying)
                .expect("idle -> verifying");
            machine.set_percent(2);
            emit_progress(sink, &machine, &mut last);
            verify_payloads(&self.manifest, payloads)?;

            machine
                .transition(Phase::Preparing)
                .expect("verifying -> preparing");
            machine.set_percent(5);
            emit_progress(sink, &machine, &mut last);
            transport
                .enter_flash_mode(self.flash_mode())
                .map_err(transport_error)?;

            machine
                .transition(Phase::Erasing)
                .expect("preparing -> erasing");
            let count = self.manifest.partitions.len().max(1);
            let mut erased = 0usize;
            for partition in &self.manifest.partitions {
                check_cancel(machine.snapshot(), sink, cancel)?;
                transport.erase(partition).map_err(transport_error)?;
                erased += 1;
                machine.set_percent(10 + (10 * erased / count) as u8);
                emit_progress(sink, &machine, &mut last);
            }

            machine
                .transition(Phase::Writing)
                .expect("erasing -> writing");
            let total = self.manifest.total_size().max(1);
            let mut written_total = 0u64;
            let mut results = Vec::with_capacity(self.manifest.partitions.len());
            for (index, partition) in self.manifest.partitions.iter().enumerate() {
                let bytes = payloads
                    .get(&partition.file_name)
                    .ok_or_else(|| ImageError::MissingFile(partition.file_name.clone()))?;
                sink.event(UpdateEvent::PartitionStarted {
                    partition: partition.name.clone(),
                    index,
                    total: self.manifest.partitions.len(),
                });
                machine.set_partition(Some(partition.name.clone()));
                let mut offset = 0usize;
                while offset < bytes.len() {
                    let end = (offset + WRITE_CHUNK).min(bytes.len());
                    transport
                        .write(&partition.name, offset as u32, &bytes[offset..end])
                        .map_err(transport_error)?;
                    let chunk = (end - offset) as u64;
                    written_total += chunk;
                    offset = end;
                    let percent = 20 + (70 * written_total / total) as u8;
                    machine.set_percent(percent.min(90));
                    emit_progress(sink, &machine, &mut last);
                    check_cancel(machine.snapshot(), sink, cancel)?;
                }
                let checksum = crc32::crc32(bytes);
                sink.event(UpdateEvent::PartitionWritten {
                    partition: partition.name.clone(),
                    crc32: checksum,
                    bytes: bytes.len() as u64,
                });
                results.push(PartitionOutcome {
                    name: partition.name.clone(),
                    bytes_written: bytes.len() as u64,
                    crc32: checksum,
                });
            }

            machine
                .transition(Phase::Validating)
                .expect("writing -> validating");
            machine.set_percent(92);
            emit_progress(sink, &machine, &mut last);
            for (partition, expected) in self.manifest.partitions.iter().zip(&results) {
                let read_back = transport
                    .read_back(&partition.name)
                    .map_err(transport_error)?;
                if read_back.len() as u64 != expected.bytes_written {
                    return Err(ImageError::SizeMismatch {
                        partition: partition.name.clone(),
                        declared: expected.bytes_written,
                        actual: read_back.len() as u64,
                    }
                    .into());
                }
                let actual = crc32::crc32(&read_back);
                if actual != partition.checksum {
                    return Err(ImageError::ChecksumMismatch {
                        partition: partition.name.clone(),
                        declared: partition.checksum,
                        actual,
                    }
                    .into());
                }
                sink.event(UpdateEvent::PartitionVerified {
                    partition: partition.name.clone(),
                    crc32: actual,
                });
            }

            machine
                .transition(Phase::Applying)
                .expect("validating -> applying");
            machine.set_percent(96);
            emit_progress(sink, &machine, &mut last);
            transport.reboot().map_err(transport_error)?;

            machine
                .transition(Phase::Reconnecting)
                .expect("applying -> reconnecting");
            machine.set_percent(99);
            emit_progress(sink, &machine, &mut last);
            let revision = transport
                .wait_for_reconnect(RECONNECT_TIMEOUT_MS)
                .map_err(transport_error)?;
            if revision.board != self.board {
                return Err(Error::Transport(format!(
                    "device came back as board {} after flashing {}",
                    revision.board, self.board
                )));
            }
            if revision.version.code != self.to.code {
                return Err(Error::Transport(format!(
                    "device came back as {} but {} was written",
                    revision.version, self.to
                )));
            }

            machine
                .transition(Phase::Done)
                .expect("reconnecting -> done");
            machine.set_percent(100);
            emit_progress(sink, &machine, &mut last);

            let mut persisted = PersistedUpdateState::new(device_id);
            persisted.required = self.policy.is_required();
            persisted.started = true;
            persisted.progress = 100;
            persisted.completed = true;
            persisted.status = crate::state::FirmwareUpdateStatus::Complete;
            sink.event(UpdateEvent::Analytics(AnalyticsEvent::Completed));
            sink.event(UpdateEvent::Persisted(persisted.clone()));

            Ok(UpdateOutcome {
                board: self.board.clone(),
                from: self.from.clone(),
                to: self.to.clone(),
                policy: self.policy,
                bytes_written: written_total,
                partitions: results,
                device_id: persisted.device_id.clone(),
                persisted,
            })
        })();

        match outcome {
            Ok(outcome) => {
                sink.event(UpdateEvent::Finished {
                    bytes_written: outcome.bytes_written,
                    version: outcome.to.clone(),
                });
                Ok(outcome)
            }
            Err(err) => {
                let failed_phase = machine.phase();
                let terminal = if matches!(err, Error::Cancelled) {
                    Phase::Cancelled
                } else {
                    Phase::Failed
                };
                let _ = machine.transition(terminal);
                sink.event(UpdateEvent::Failed {
                    phase: failed_phase,
                    message: err.to_string(),
                });
                Err(err)
            }
        }
    }
}

fn emit_progress<S: EventSink>(
    sink: &mut S,
    machine: &UpdateStateMachine,
    last: &mut Option<(u8, Phase)>,
) {
    let key = (machine.percent(), machine.phase());
    if *last != Some(key) {
        *last = Some(key);
        sink.event(UpdateEvent::Progress(machine.snapshot()));
    }
}

fn check_cancel<S, F>(progress: Progress, sink: &S, cancel: &mut F) -> Result<()>
where
    S: EventSink,
    F: FnMut(&Progress) -> bool,
{
    if sink.cancelled() {
        return Err(Error::Cancelled);
    }
    if cancel(&progress) {
        return Err(Error::Cancelled);
    }
    Ok(())
}

fn transport_error<E: fmt::Display>(err: E) -> Error {
    Error::Transport(err.to_string())
}

/// Checks a device against a manifest and decides whether to update.
///
/// Mirrors `DiagnosticsCheckFirmwareUseCase`: the manifest must be valid, it
/// must target the device's board, and the versions must differ (unless the
/// gate forces an update or allows a reinstall).
pub fn check_firmware(
    device: &DeviceInfo,
    manifest: &OtaManifest,
    gate: &UpdateGate,
) -> Result<PlanDecision> {
    manifest.validate()?;
    if device.board != manifest.version.board_name {
        return Err(ImageError::BoardMismatch {
            device: device.board.clone(),
            manifest: manifest.version.board_name.clone(),
        }
        .into());
    }

    let latest = manifest.version.version.clone();
    let current = device.version.clone();

    let choice: PolicyChoice = if gate.forced {
        Some((UpdatePolicy::Required, UpdateReason::Forced))
    } else if let Some(minimum) = &gate.minimum {
        if current < *minimum {
            Some((
                UpdatePolicy::Required,
                UpdateReason::BelowMinimum {
                    minimum: minimum.clone(),
                },
            ))
        } else {
            choose_policy(&current, &latest, gate.allow_reinstall)
        }
    } else {
        choose_policy(&current, &latest, gate.allow_reinstall)
    };

    match choice {
        Some((policy, reason)) => {
            let profile = device::board_profile(&device.board);
            Ok(PlanDecision::Update(Box::new(UpdatePlan {
                board: manifest.version.board_name.clone(),
                family: profile.map(|p| p.family).unwrap_or(DeviceFamily::Generic),
                updater: device.updater(),
                from: current,
                to: latest,
                policy,
                reason,
                total_bytes: manifest.total_size(),
                manifest: manifest.clone(),
            })))
        }
        None => {
            let reason = if current > latest {
                UpdateReason::NewerOnDevice
            } else {
                UpdateReason::UpToDate
            };
            Ok(PlanDecision::UpToDate {
                current,
                latest,
                reason,
            })
        }
    }
}

type PolicyChoice = Option<(UpdatePolicy, UpdateReason)>;

fn choose_policy(current: &FwVersion, latest: &FwVersion, allow_reinstall: bool) -> PolicyChoice {
    if current < latest {
        return Some((UpdatePolicy::Recommended, UpdateReason::NewerAvailable));
    }
    if current == latest && allow_reinstall {
        return Some((UpdatePolicy::Optional, UpdateReason::Reinstall));
    }
    None
}

/// A set of downloaded firmware payloads keyed by `file_name`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PayloadSet {
    files: BTreeMap<String, Vec<u8>>,
    /// Device id used for persistence when the set is executed directly.
    pub device_id: Option<String>,
}

impl PayloadSet {
    /// Empty set.
    pub fn new() -> Self {
        PayloadSet::default()
    }

    /// Registers one payload file.
    pub fn insert(&mut self, name: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.files.insert(name.into(), bytes.into());
    }

    /// Builder form of [`PayloadSet::insert`].
    pub fn with(mut self, name: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        self.insert(name, bytes);
        self
    }

    /// Sets the device id recorded in persistence.
    pub fn for_device(mut self, id: impl Into<String>) -> Self {
        self.device_id = Some(id.into());
        self
    }

    /// Looks up a file.
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name).map(Vec::as_slice)
    }

    /// Number of files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// True when no files were registered.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// File names, sorted.
    pub fn names(&self) -> Vec<&str> {
        self.files.keys().map(String::as_str).collect()
    }

    /// Total payload bytes.
    pub fn total_bytes(&self) -> u64 {
        self.files.values().map(|v| v.len() as u64).sum()
    }

    /// Loads every regular file in `dir` as a payload.
    ///
    /// Manifest XML files are skipped so a mirrored package directory can be
    /// pointed at directly.
    pub fn from_dir(dir: impl AsRef<std::path::Path>) -> Result<PayloadSet> {
        let dir = dir.as_ref();
        let entries = std::fs::read_dir(dir)
            .map_err(|e| Error::Download(format!("{}: {e}", dir.display())))?;
        let mut set = PayloadSet::new();
        for entry in entries {
            let entry = entry.map_err(|e| Error::Download(format!("{}: {e}", dir.display())))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };
            if name.to_ascii_lowercase().ends_with(".xml") {
                continue;
            }
            let bytes = std::fs::read(&path)
                .map_err(|e| Error::Download(format!("{}: {e}", path.display())))?;
            set.insert(name, bytes);
        }
        Ok(set)
    }
}

/// A partition that was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionOutcome {
    /// Partition name from the manifest.
    pub name: String,
    /// Bytes written.
    pub bytes_written: u64,
    /// CRC-32 of the bytes.
    pub crc32: u32,
}

/// Result of a successful update.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateOutcome {
    /// Board that was flashed.
    pub board: String,
    /// Version before the update.
    pub from: FwVersion,
    /// Version after the update.
    pub to: FwVersion,
    /// Policy that drove the update.
    pub policy: UpdatePolicy,
    /// Total payload bytes written.
    pub bytes_written: u64,
    /// Per-partition results.
    pub partitions: Vec<PartitionOutcome>,
    /// Persistence key of the device.
    pub device_id: String,
    /// Final persisted state, as the Dart layer would store it.
    pub persisted: PersistedUpdateState,
}

/// Validates every manifest partition against the payload set.
pub fn verify_payloads(manifest: &OtaManifest, payloads: &PayloadSet) -> Result<()> {
    manifest.validate()?;
    for partition in &manifest.partitions {
        let bytes = payloads
            .get(&partition.file_name)
            .ok_or_else(|| ImageError::MissingFile(partition.file_name.clone()))?;
        let actual_size = bytes.len() as u64;
        if actual_size != u64::from(partition.file_size) {
            return Err(ImageError::SizeMismatch {
                partition: partition.name.clone(),
                declared: u64::from(partition.file_size),
                actual: actual_size,
            }
            .into());
        }
        let actual = crc32::crc32(bytes);
        if actual != partition.checksum {
            return Err(ImageError::ChecksumMismatch {
                partition: partition.name.clone(),
                declared: partition.checksum,
                actual,
            }
            .into());
        }
    }
    validate_layout(manifest)?;
    Ok(())
}

/// Enforces the partition layout the embedded manifests use.
///
/// Nordic families always ship `BOOT` (`mbrec.bin`) plus `SYSTEM`
/// (`zephyr.bin`); the `wm_*` family always ships `SYSTEM`, `DATA`, `BOOT` and
/// `SYS_PARAM` with `storage_id` present. A manifest that disagrees with the
/// board's family is rejected before anything touches the device, which is the
/// structural half of the Dart `Invalid Nordic firmware.` status. The
/// byte-level half (nRF vector table / DFU init packet) is not recoverable and
/// is left to the transport.
pub fn validate_layout(manifest: &OtaManifest) -> std::result::Result<(), ManifestError> {
    let family = device::board_profile(&manifest.version.board_name).map(|p| p.family);
    let has = |t: PartitionType| manifest.partitions.iter().any(|p| p.ptype == t);

    match family {
        Some(f) if f.is_nordic() => {
            if !has(PartitionType::Boot) || !has(PartitionType::System) {
                return Err(ManifestError::Layout(format!(
                    "Nordic board {} needs BOOT and SYSTEM partitions",
                    manifest.version.board_name
                )));
            }
        }
        Some(DeviceFamily::Wm) => {
            for required in [
                PartitionType::System,
                PartitionType::Data,
                PartitionType::Boot,
                PartitionType::SysParam,
            ] {
                if !has(required.clone()) {
                    return Err(ManifestError::Layout(format!(
                        "board {} is missing its {} partition",
                        manifest.version.board_name,
                        required.as_str()
                    )));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// A device revision reported after flashing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareRevision {
    /// Board id the device reports.
    pub board: String,
    /// Version the device reports.
    pub version: FwVersion,
}

impl FirmwareRevision {
    /// Builds a revision.
    pub fn new(board: impl Into<String>, version: FwVersion) -> Self {
        FirmwareRevision {
            board: board.into(),
            version,
        }
    }
}

/// The seam a real port must implement: byte-level access to the device.
///
/// None of this is recoverable from the shipped artifacts (see `docs/GAPS.md`),
/// so the manager only depends on the *sequence* of operations, never on the
/// packet format.
pub trait FirmwareTransport {
    /// Transport error type.
    type Error: fmt::Display;

    /// Puts the device into bootloader / DFU mode.
    fn enter_flash_mode(&mut self, mode: FlashMode) -> std::result::Result<(), Self::Error>;

    /// Erases the region backing `partition`.
    fn erase(&mut self, partition: &Partition) -> std::result::Result<(), Self::Error>;

    /// Writes `data` at `offset` inside the named partition.
    fn write(
        &mut self,
        partition: &str,
        offset: u32,
        data: &[u8],
    ) -> std::result::Result<(), Self::Error>;

    /// Reads the partition back for verification.
    fn read_back(&mut self, partition: &str) -> std::result::Result<Vec<u8>, Self::Error>;

    /// Reboots into the written image.
    fn reboot(&mut self) -> std::result::Result<FirmwareRevision, Self::Error>;

    /// Waits for the device to re-enumerate and reports its revision.
    fn wait_for_reconnect(
        &mut self,
        timeout_ms: u64,
    ) -> std::result::Result<FirmwareRevision, Self::Error>;
}

/// One operation recorded by [`SimulatedTransport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimulatedOp {
    /// Flash mode was entered.
    EnterFlashMode(FlashMode),
    /// A partition was erased.
    Erase(String),
    /// Bytes were written.
    Write {
        /// Partition name.
        partition: String,
        /// Byte offset.
        offset: u32,
        /// Number of bytes.
        len: usize,
    },
    /// A partition was read back.
    ReadBack(String),
    /// The device was rebooted.
    Reboot,
    /// A reconnect was awaited.
    WaitForReconnect,
}

impl fmt::Display for SimulatedOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimulatedOp::EnterFlashMode(mode) => write!(f, "enter-flash-mode {mode}"),
            SimulatedOp::Erase(name) => write!(f, "erase {name}"),
            SimulatedOp::Write {
                partition,
                offset,
                len,
            } => write!(f, "write {partition} @{offset} {len}B"),
            SimulatedOp::ReadBack(name) => write!(f, "read-back {name}"),
            SimulatedOp::Reboot => f.write_str("reboot"),
            SimulatedOp::WaitForReconnect => f.write_str("wait-for-reconnect"),
        }
    }
}

/// In-memory device used by tests and `rode-fw simulate`.
#[derive(Debug, Clone)]
pub struct SimulatedTransport {
    revision: FirmwareRevision,
    mode: Option<FlashMode>,
    partitions: BTreeMap<String, Vec<u8>>,
    log: Vec<SimulatedOp>,
    failures: BTreeMap<String, String>,
    rebooted: u32,
}

impl SimulatedTransport {
    /// A blank device reporting `board` / `version`.
    pub fn new(board: impl Into<String>, version: FwVersion) -> Self {
        SimulatedTransport {
            revision: FirmwareRevision::new(board, version),
            mode: None,
            partitions: BTreeMap::new(),
            log: Vec::new(),
            failures: BTreeMap::new(),
            rebooted: 0,
        }
    }

    /// Overrides the revision reported after a reboot.
    pub fn set_revision(&mut self, revision: FirmwareRevision) {
        self.revision = revision;
    }

    /// Current reported revision.
    pub fn revision(&self) -> &FirmwareRevision {
        &self.revision
    }

    /// Flash mode that was entered, if any.
    pub fn mode(&self) -> Option<FlashMode> {
        self.mode
    }

    /// Recorded operations.
    pub fn log(&self) -> &[SimulatedOp] {
        &self.log
    }

    /// Log rendered one operation per line.
    pub fn log_lines(&self) -> Vec<String> {
        self.log.iter().map(ToString::to_string).collect()
    }

    /// Contents of a partition after flashing.
    pub fn partition(&self, name: &str) -> Option<&[u8]> {
        self.partitions.get(name).map(Vec::as_slice)
    }

    /// All partition names written so far.
    pub fn partition_names(&self) -> Vec<&str> {
        self.partitions.keys().map(String::as_str).collect()
    }

    /// How many times `reboot` was called.
    pub fn reboot_count(&self) -> u32 {
        self.rebooted
    }

    /// Injects a failure. Keys are `enter_flash_mode`, `erase:<partition>`,
    /// `write:<partition>`, `read_back:<partition>`, `reboot` and
    /// `wait_for_reconnect`.
    pub fn fail_on(&mut self, key: impl Into<String>, message: impl Into<String>) {
        self.failures.insert(key.into(), message.into());
    }

    fn failure(&self, key: &str) -> std::result::Result<(), String> {
        match self.failures.get(key) {
            Some(message) => Err(message.clone()),
            None => Ok(()),
        }
    }
}

impl FirmwareTransport for SimulatedTransport {
    type Error = String;

    fn enter_flash_mode(&mut self, mode: FlashMode) -> std::result::Result<(), String> {
        self.failure("enter_flash_mode")?;
        self.log.push(SimulatedOp::EnterFlashMode(mode));
        self.mode = Some(mode);
        Ok(())
    }

    fn erase(&mut self, partition: &Partition) -> std::result::Result<(), String> {
        self.failure(&format!("erase:{}", partition.name))?;
        self.log.push(SimulatedOp::Erase(partition.name.clone()));
        self.partitions.remove(&partition.name);
        Ok(())
    }

    fn write(
        &mut self,
        partition: &str,
        offset: u32,
        data: &[u8],
    ) -> std::result::Result<(), String> {
        self.failure(&format!("write:{partition}"))?;
        self.log.push(SimulatedOp::Write {
            partition: partition.to_string(),
            offset,
            len: data.len(),
        });
        let buffer = self.partitions.entry(partition.to_string()).or_default();
        let offset = offset as usize;
        if buffer.len() < offset + data.len() {
            buffer.resize(offset + data.len(), 0);
        }
        buffer[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    fn read_back(&mut self, partition: &str) -> std::result::Result<Vec<u8>, String> {
        self.failure(&format!("read_back:{partition}"))?;
        self.log.push(SimulatedOp::ReadBack(partition.to_string()));
        Ok(self.partitions.get(partition).cloned().unwrap_or_default())
    }

    fn reboot(&mut self) -> std::result::Result<FirmwareRevision, String> {
        self.failure("reboot")?;
        self.log.push(SimulatedOp::Reboot);
        self.rebooted += 1;
        Ok(self.revision.clone())
    }

    fn wait_for_reconnect(
        &mut self,
        _timeout_ms: u64,
    ) -> std::result::Result<FirmwareRevision, String> {
        self.failure("wait_for_reconnect")?;
        self.log.push(SimulatedOp::WaitForReconnect);
        Ok(self.revision.clone())
    }
}

/// Events emitted during an update.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateEvent {
    /// Progress snapshot; drives the `void(int)` / `void(double)` signals.
    Progress(Progress),
    /// Analytics event the Dart layer would log.
    Analytics(AnalyticsEvent),
    /// A partition is about to be written.
    PartitionStarted {
        /// Partition name.
        partition: String,
        /// Zero-based index.
        index: usize,
        /// Number of partitions.
        total: usize,
    },
    /// A partition finished writing.
    PartitionWritten {
        /// Partition name.
        partition: String,
        /// CRC-32 of the bytes written.
        crc32: u32,
        /// Bytes written.
        bytes: u64,
    },
    /// A partition passed read-back verification.
    PartitionVerified {
        /// Partition name.
        partition: String,
        /// CRC-32 read back from the device.
        crc32: u32,
    },
    /// Persisted state that the Dart layer would store.
    Persisted(PersistedUpdateState),
    /// Update finished successfully.
    Finished {
        /// Total bytes written.
        bytes_written: u64,
        /// Installed version.
        version: FwVersion,
    },
    /// Update failed or was cancelled.
    Failed {
        /// Phase the failure was observed in.
        phase: Phase,
        /// Error text.
        message: String,
    },
}

impl UpdateEvent {
    /// One-line rendering for CLI output.
    pub fn describe(&self) -> String {
        match self {
            UpdateEvent::Progress(p) => match &p.partition {
                Some(name) => format!("{} {}% {} ({name})", p.status, p.percent, p.phase.as_str()),
                None => format!("{} {}% {}", p.status, p.percent, p.phase.as_str()),
            },
            UpdateEvent::Analytics(e) => format!("analytics {}", e.as_str()),
            UpdateEvent::PartitionStarted {
                partition,
                index,
                total,
            } => format!("partition {}/{total} {partition}", index + 1),
            UpdateEvent::PartitionWritten {
                partition,
                crc32,
                bytes,
            } => format!("wrote {partition} {bytes}B crc32=0x{crc32:08x}"),
            UpdateEvent::PartitionVerified { partition, crc32 } => {
                format!("verified {partition} crc32=0x{crc32:08x}")
            }
            UpdateEvent::Persisted(state) => format!("persist {}", state.to_json()),
            UpdateEvent::Finished {
                bytes_written,
                version,
            } => format!("finished {version} {bytes_written}B"),
            UpdateEvent::Failed { phase, message } => match phase {
                Phase::Failed => format!("failed: {message}"),
                Phase::Cancelled => format!("cancelled: {message}"),
                other => format!("failed in {}: {message}", other.as_str()),
            },
        }
    }
}

/// Receives [`UpdateEvent`]s; the analogue of the bridge's signal slots.
pub trait EventSink {
    /// Handles one event.
    fn event(&mut self, event: UpdateEvent);

    /// Polled between partitions; return `true` to abort like
    /// `cancelFirmwareUpdate`.
    fn cancelled(&self) -> bool {
        false
    }
}

/// Sink that discards everything.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullSink;

impl EventSink for NullSink {
    fn event(&mut self, _event: UpdateEvent) {}
}

/// Sink that records events for tests and the CLI.
#[derive(Debug, Clone, Default)]
pub struct EventLog {
    events: Vec<UpdateEvent>,
}

impl EventLog {
    /// Fresh, empty log.
    pub fn new() -> Self {
        EventLog::default()
    }

    /// Recorded events.
    pub fn events(&self) -> &[UpdateEvent] {
        &self.events
    }

    /// Consumes the log.
    pub fn into_events(self) -> Vec<UpdateEvent> {
        self.events
    }

    /// Last progress snapshot, if any.
    pub fn last_progress(&self) -> Option<&Progress> {
        self.events.iter().rev().find_map(|e| match e {
            UpdateEvent::Progress(p) => Some(p),
            _ => None,
        })
    }

    /// Analytics events recorded, in order.
    pub fn analytics(&self) -> Vec<AnalyticsEvent> {
        self.events
            .iter()
            .filter_map(|e| match e {
                UpdateEvent::Analytics(a) => Some(*a),
                _ => None,
            })
            .collect()
    }

    /// True when a failure was recorded.
    pub fn failed(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, UpdateEvent::Failed { .. }))
    }
}

impl EventSink for EventLog {
    fn event(&mut self, event: UpdateEvent) {
        self.events.push(event);
    }
}

/// The bridge's `FirmwareUpdateManager`: planning plus download helpers.
pub struct FirmwareUpdateManager {
    downloader: Box<dyn Downloader>,
}

impl fmt::Debug for FirmwareUpdateManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FirmwareUpdateManager")
            .finish_non_exhaustive()
    }
}

impl FirmwareUpdateManager {
    /// Builds a manager around any [`Downloader`].
    pub fn new(downloader: Box<dyn Downloader>) -> Self {
        FirmwareUpdateManager { downloader }
    }

    /// Builds a manager that resolves URLs to files in `root`.
    pub fn with_directory(root: impl Into<std::path::PathBuf>) -> Self {
        FirmwareUpdateManager::new(Box::new(crate::download::DirectoryDownloader::new(root)))
    }

    /// Builds a manager that talks plain HTTP.
    pub fn with_http() -> Self {
        FirmwareUpdateManager::new(Box::new(crate::download::HttpFileDownloader))
    }

    /// Decides whether `device` should be updated to `manifest`.
    pub fn check_firmware(
        &self,
        device: &DeviceInfo,
        manifest: &OtaManifest,
        gate: &UpdateGate,
    ) -> Result<PlanDecision> {
        check_firmware(device, manifest, gate)
    }

    /// Fetches and parses a download manifest (`*-manifest`).
    pub fn download_manifest(&self, url: &str) -> Result<FirmwareManifest> {
        let bytes = self.downloader.get(url)?;
        let text = String::from_utf8_lossy(&bytes);
        Ok(FirmwareManifest::parse(&text)?)
    }

    /// Fetches the firmware package described by `manifest` and checks
    /// `firmware-md5`.
    pub fn download_firmware(&self, manifest: &FirmwareManifest) -> Result<Vec<u8>> {
        let bytes = self.downloader.get(&manifest.firmware_url)?;
        manifest.verify_firmware(&bytes)?;
        Ok(bytes)
    }

    /// Runs a plan, delegating to [`UpdatePlan::execute`].
    pub fn run<T, S>(
        &self,
        plan: &UpdatePlan,
        payloads: &PayloadSet,
        transport: &mut T,
        sink: &mut S,
    ) -> Result<UpdateOutcome>
    where
        T: FirmwareTransport,
        S: EventSink,
    {
        plan.execute(payloads, transport, sink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::ota_xml::XmlStyle;

    fn parsed(board: &str) -> OtaManifest {
        let xml = fixtures::ALL
            .iter()
            .find(|(b, _)| *b == board)
            .map(|(_, xml)| *xml)
            .unwrap_or(fixtures::WGO2_RX_1_93);
        OtaManifest::parse(xml).unwrap()
    }

    /// Rebuilds a fixture so its declared sizes/checksums match synthetic payloads.
    fn manifest_with(manifest: &OtaManifest, payloads: &PayloadSet) -> OtaManifest {
        let mut m = manifest.clone();
        for p in &mut m.partitions {
            let bytes = payloads.get(&p.file_name).expect("payload registered");
            p.file_size = bytes.len() as u32;
            p.checksum = crc32::crc32(bytes);
        }
        m
    }

    fn wgo2_plan() -> (UpdatePlan, PayloadSet) {
        let base = parsed("wgo2_rx");
        let payloads = PayloadSet::new()
            .with("mbrec.bin", vec![0x11u8; 64])
            .with(
                "zephyr.bin",
                (0..300u32).map(|i| i as u8).collect::<Vec<u8>>(),
            )
            .for_device("SN-TEST");
        let manifest = manifest_with(&base, &payloads);
        let device = DeviceInfo::new("wgo2_rx", FwVersion::from_name("1.80").unwrap())
            .with_serial("SN-TEST");
        let decision = check_firmware(&device, &manifest, &UpdateGate::normal()).unwrap();
        let plan = decision.plan().expect("update planned").clone();
        (plan, payloads)
    }

    #[test]
    fn fixture_planning_matches_versions() {
        for (board, xml) in fixtures::ALL {
            let manifest = OtaManifest::parse(xml).unwrap();
            let old = FwVersion::new("0.1", 0x01);
            let device = DeviceInfo::new(*board, old);
            let decision = check_firmware(&device, &manifest, &UpdateGate::normal()).unwrap();
            let plan = decision.plan().expect("older device must update");
            assert_eq!(plan.to, *manifest.version());
            assert_eq!(plan.policy, UpdatePolicy::Recommended);

            let current = DeviceInfo::new(*board, manifest.version().clone());
            let decision = check_firmware(&current, &manifest, &UpdateGate::normal()).unwrap();
            assert!(matches!(
                decision,
                PlanDecision::UpToDate {
                    reason: UpdateReason::UpToDate,
                    ..
                }
            ));

            let forced = check_firmware(&current, &manifest, &UpdateGate::forced()).unwrap();
            assert_eq!(forced.plan().unwrap().policy, UpdatePolicy::Required);
        }
    }

    #[test]
    fn board_mismatch_is_rejected() {
        let manifest = parsed("wgo2_rx");
        let device = DeviceInfo::new("wme_rx", FwVersion::from_name("1.0").unwrap());
        let err = check_firmware(&device, &manifest, &UpdateGate::normal()).unwrap_err();
        assert!(matches!(
            err,
            Error::Image(ImageError::BoardMismatch { .. })
        ));
    }

    #[test]
    fn pipeline_flashes_and_verifies() {
        let (plan, payloads) = wgo2_plan();
        let mut transport =
            SimulatedTransport::new("wgo2_rx", FwVersion::from_name("1.80").unwrap());
        transport.set_revision(FirmwareRevision::new("wgo2_rx", plan.to.clone()));
        let mut sink = EventLog::new();

        let outcome = plan.execute(&payloads, &mut transport, &mut sink).unwrap();
        assert_eq!(outcome.to.name, "1.93");
        assert_eq!(outcome.bytes_written, 364);
        assert_eq!(transport.mode(), Some(FlashMode::Normal));
        assert_eq!(transport.reboot_count(), 1);
        assert_eq!(
            transport.partition("fw0_sys").unwrap(),
            payloads.get("zephyr.bin").unwrap()
        );
        assert_eq!(sink.last_progress().unwrap().percent, 100);
        assert!(!sink.failed());
        assert_eq!(
            sink.analytics(),
            vec![AnalyticsEvent::Started, AnalyticsEvent::Completed]
        );
        let persisted = &outcome.persisted;
        assert!(persisted.completed);
        assert_eq!(
            persisted.status,
            crate::state::FirmwareUpdateStatus::Complete
        );
    }

    #[test]
    fn wrong_checksum_aborts_before_touching_the_device() {
        let (plan, mut payloads) = wgo2_plan();
        payloads.insert("zephyr.bin", vec![0u8; 300]);
        let mut transport = SimulatedTransport::new("wgo2_rx", plan.from.clone());
        let mut sink = EventLog::new();
        let err = plan
            .execute(&payloads, &mut transport, &mut sink)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::Image(ImageError::ChecksumMismatch { .. })
        ));
        assert!(transport.log().is_empty());
        assert!(sink.failed());
    }

    #[test]
    fn missing_payload_is_reported() {
        let (plan, mut payloads) = wgo2_plan();
        payloads = PayloadSet {
            files: payloads
                .files
                .into_iter()
                .filter(|(k, _)| k != "mbrec.bin")
                .collect(),
            device_id: payloads.device_id,
        };
        let mut transport = SimulatedTransport::new("wgo2_rx", plan.from.clone());
        let err = plan
            .execute(&payloads, &mut transport, &mut NullSink)
            .unwrap_err();
        assert!(matches!(err, Error::Image(ImageError::MissingFile(_))));
    }

    #[test]
    fn transport_failure_marks_failed() {
        let (plan, payloads) = wgo2_plan();
        let mut transport = SimulatedTransport::new("wgo2_rx", plan.from.clone());
        transport.fail_on("erase:fw0_boot", "device NAK");
        let mut sink = EventLog::new();
        let err = plan
            .execute(&payloads, &mut transport, &mut sink)
            .unwrap_err();
        assert_eq!(err, Error::Transport("device NAK".into()));
        assert!(matches!(
            sink.events().last().unwrap(),
            UpdateEvent::Failed {
                phase: Phase::Erasing,
                ..
            }
        ));
    }

    #[test]
    fn reboot_reporting_wrong_version_fails() {
        let (plan, payloads) = wgo2_plan();
        let mut transport =
            SimulatedTransport::new("wgo2_rx", FwVersion::from_name("1.80").unwrap());
        transport.set_revision(FirmwareRevision::new(
            "wgo2_rx",
            FwVersion::from_name("1.81").unwrap(),
        ));
        let err = plan
            .execute(&payloads, &mut transport, &mut NullSink)
            .unwrap_err();
        assert!(matches!(err, Error::Transport(_)));
    }

    #[test]
    fn forced_update_is_required_and_uses_forced_flash_mode() {
        let base = parsed("wgo2_rx");
        let payloads = PayloadSet::new()
            .with("mbrec.bin", vec![1u8; 8])
            .with("zephyr.bin", vec![2u8; 8]);
        let manifest = manifest_with(&base, &payloads);
        let device = DeviceInfo::new("wgo2_rx", manifest.version().clone());
        let decision = check_firmware(&device, &manifest, &UpdateGate::forced()).unwrap();
        let plan = decision.plan().unwrap();
        assert_eq!(plan.policy, UpdatePolicy::Required);
        assert!(plan.flash_mode() == FlashMode::Forced);
        assert_eq!(plan.reason, UpdateReason::Forced);
    }

    #[test]
    fn minimum_version_makes_update_required() {
        let base = parsed("wgo2_rx");
        let minimum = FwVersion::from_name("1.99").unwrap();
        let device = DeviceInfo::new("wgo2_rx", FwVersion::from_name("1.93").unwrap());
        let gate = UpdateGate::normal().with_minimum(minimum.clone());
        let decision = check_firmware(&device, &base, &gate).unwrap();
        let plan = decision.plan().unwrap();
        assert_eq!(plan.policy, UpdatePolicy::Required);
        assert_eq!(plan.reason, UpdateReason::BelowMinimum { minimum });
    }

    #[test]
    fn reinstall_is_optional() {
        let base = parsed("wgo2_rx");
        let device = DeviceInfo::new("wgo2_rx", base.version().clone());
        let gate = UpdateGate {
            allow_reinstall: true,
            ..UpdateGate::default()
        };
        let decision = check_firmware(&device, &base, &gate).unwrap();
        assert_eq!(decision.plan().unwrap().policy, UpdatePolicy::Optional);
    }

    #[test]
    fn layout_validation_rejects_missing_partitions() {
        let base = parsed("wm_tx");
        let mut broken = base.clone();
        broken
            .partitions
            .retain(|p| p.ptype != PartitionType::SysParam);
        broken.partitions_num = broken.partitions.len() as u32;
        assert!(validate_layout(&broken).is_err());
        assert!(validate_layout(&base).is_ok());
    }

    #[test]
    fn payload_set_can_hold_a_single_image() {
        let set = PayloadSet::new().with("NT-USB_Mini v2.3.6.bin", vec![9u8; 16]);
        assert_eq!(set.len(), 1);
        assert_eq!(set.total_bytes(), 16);
        assert_eq!(set.names(), vec!["NT-USB_Mini v2.3.6.bin"]);
    }

    #[test]
    fn manifest_round_trips_after_rebuild() {
        let base = parsed("wgo2_rx");
        let payloads = PayloadSet::new()
            .with("mbrec.bin", vec![0u8; 32])
            .with("zephyr.bin", vec![0xffu8; 32]);
        let manifest = manifest_with(&base, &payloads);
        let xml = manifest.to_xml(XmlStyle::Tabbed);
        let again = OtaManifest::parse(&xml).unwrap();
        assert_eq!(again, manifest);
    }
}
