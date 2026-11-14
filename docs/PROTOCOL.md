# Recovered update protocol

This document describes the firmware-update formats and ordering implemented by
this crate. Each statement is tagged:

* **[P]** proven by bytes in the shipped artifacts (string, RTTI name, table or
  manifest);
* **[I]** inferred from several proven facts;
* **[R]** reconstruction where the artifacts are silent (tracked in
  [`GAPS.md`](GAPS.md)).

All addresses are file-mapped virtual addresses for `rode_bridge.dll`, image
base `0x180000000`.

---

## 1. Where the feature lives

The application splits the feature across two artifacts.

### Dart / Flutter layer (`data/app.so`)

UI, cubits, use cases and persistence. The following use cases are present by
name **[P]**:

```text
SetFirmwareUpdateToLocalBinary
UpdateFirmwareDevice
UpdateFirmwareDeviceWithSessionInfo
CompleteFirmwareUpdate
DiagnosticsCheckFirmwareUseCase
FirmwareUpdateInstallingSubjectUseCase
CancelFirmwareUpdate
ForceFirmwareUpdate
```

and the bridge calls they make are **[P]**:

```text
setFirmwareUpdateToLocalBinary   diagnosticsCheckFirmware
updateFirmwareDevice             updateFirmwareDeviceWithSessionInfo
completeFirmwareUpdate           cancelFirmwareUpdate
startFirmwareUpdate              finishFirmwareUpdate
forceFirmwareUpdate              checkFirmware
setFlashing                      setOtaSupport
setNthwUsbMicMode                getConnectedDevices
setSessionIgnoreConnection
```

`app.so` is a Dart AOT snapshot: it exports entry points but has no native
procedure metadata, so it is an orchestration oracle, not a decompilation
target.

### Native layer (`rode_bridge.dll`)

`FirmwareUpdater` is a base class with a concrete hierarchy recovered from MSVC
RTTI type descriptors **[P]**:

```text
FirmwareUpdater
├── IntProFirmwareUpdater
├── WirelessFirmwareUpdater
│   ├── WGo2FirmwareUpdater
│   ├── WGo3FirmwareUpdater
│   ├── WMeFirmwareUpdater
│   ├── WProFirmwareUpdater
│   ├── WMicroFirmwareUpdater
│   └── FilmmakerIIFirmwareUpdater
├── DefaultFirmwareUpdater
├── GenericFirmwareUpdater
├── OtaFirmwareUpdater
└── QccHidFirmwareUpdater
```

Supporting classes: `FirmwareUpdateManager`, `FirmwareDataOTA`, `Session`,
`ManifestDownloadThread`, `DownloadTask`, `FallbackDownloadTask`, `HidDevices`,
`NthwDeviceIo`, `RodeSettingsHidTransport`.

Signals are sigslot slots whose signatures survive in `slot_pmf` RTTI **[P]**:

| Signal | Meaning | Classes observed |
|---|---|---|
| `void(int)` | integer progress (percent) | most updaters |
| `void(bool)` | completion flag | most updaters |
| `void()` | argument-less done | `GenericFirmwareUpdater` |
| `void(FirmwareUpdater::State)` | state transitions | manager/updater |
| `void(double)` | fractional progress | manager |

The crate represents these as [`UpdateEvent`](../src/manager.rs) variants.

## 2. OTA firmware manifest (XML)

Every firmware package is described by an `ota_firmware` document embedded in
`.rdata` (twelve real documents are reproduced in `src/fixtures.rs`).

### 2.1 Document shape **[P]**

```xml
<?xml version='1.0' encoding='UTF-8'?>
<ota_firmware>
  <firmware_version>
    <board_name>wgo2_rx</board_name>
    <version_name>1.93</version_name>
    <version_code>0x193</version_code>
  </firmware_version>
  <partitions>
    <partitionsNum>2</partitionsNum>
    <partition>
      <type>BOOT</type>
      <name>fw0_boot</name>
      <file_id>1</file_id>
      <file_name>mbrec.bin</file_name>
      <file_size>0x1000</file_size>
      <checksum>0x488ed61c</checksum>
    </partition>
    ...
  </partitions>
</ota_firmware>
```

* Root must be `ota_firmware`; anything else is rejected.
* `firmware_version` children: `board_name`, `version_name`, `version_code`
  and, in the four compact documents, `version_res`.
* `partitions` children: `partitionsNum` followed by exactly that many
  `partition` elements.
* `partition` children: `type`, `name`, `file_id`, `file_name`, optional
  `storage_id`, `file_size`, `checksum`.
* Numbers accept either `0x`-prefixed hex or plain decimal.

### 2.2 Two serialisation styles **[P]**

The embedded documents come in two styles and both must round-trip.

**Tabbed** (ten documents: all `wgo2*`, `wgo25*`, `wme*`):

* `board_name`, `version_name`, `version_code` in that order;
* `version_res` is absent from all eight tabbed documents;
* closing `</firmware_version>` is indented two tabs, `</partitions>` and
  `</ota_firmware>` one tab;
* hex numbers are lower-case with a `0x` prefix; `checksum` is zero-padded to
  eight digits.

**Compact** (four documents: `wm_tx`, `wm_rx`, `wm_crx`, `wmcam_rx`):

* child order is `version_code`, `version_name`, `version_res`, `board_name`;
* the empty `version_res` is written as an explicit `<version_res />`
  (all four embedded compact documents have it);
* no indentation at all.

The fixture tests assert byte-for-byte equality with the originals in both
styles.

### 2.3 `version_code` derivation **[P]**

`version_code` is **not** the version parsed as a decimal number. It is every
digit of `version_name` concatenated and read as hexadecimal. This holds for
all twelve documents:

| `version_name` | digits | `version_code` |
|---|---|---|
| `1.93` | `193` | `0x193` |
| `2.50` | `250` | `0x250` |
| `1.47` | `147` | `0x147` |
| `2.31` | `231` | `0x231` |

`FwVersion::code_from_name` implements the rule (non-digits are skipped);
`FwVersion::compare` orders numerically by dotted segments, so `1.9 < 1.10`
even though `0x19 < 0x110` happens to agree.

### 2.4 Structural validation

The parser enforces what the embedded documents guarantee:

* root element is `ota_firmware`;
* `partitionsNum == partitions.len()`, and there is at least one partition;
* no duplicate `file_id` and no duplicate partition `name`;
* every `file_size` is non-zero;
* numeric fields parse.

### 2.5 Partition catalogues **[P]**

Two layouts occur, and they correlate exactly with board prefix:

**Nordic boards** (`wgo2_*`, `wgo25_*`, `wme_*`):

| type | name | file_id | file_name | storage_id |
|---|---|---|---|---|
| `BOOT` | `fw0_boot` | 1 | `mbrec.bin` | absent |
| `SYSTEM` | `fw0_sys` | 3 | `zephyr.bin` | absent |

**`wm_*` / `wmcam_*` boards** (four partitions, all `storage_id 0`):

| type | name | file_id | file_name |
|---|---|---|---|
| `SYSTEM` | `fw0_sys` | 4 | `app.bin` |
| `DATA` | `fw0_sdfs` | 39 | `tone.bin` |
| `BOOT` | `fw0_boot` | 1 | `mbrec.bin` |
| `SYS_PARAM` | `fw0_param` | 2 | `param.bin` |

`validate_layout` rejects a manifest whose partitions do not match the layout
implied by its board. This is the structural half of the Dart
`Invalid Nordic firmware.` / `invalidNordicFw` status; the byte-level half (nRF
vector table / DFU init packet) is not recoverable.

### 2.6 Checksums are content-derived **[P, algorithm I]**

The same payload carries the same `<checksum>` across independent manifests:

* `tone.bin` is `0xd81bef6b` in all four `wm_*` documents;
* `mbrec.bin` is `0xe4d27a38` in all four `wm_*` documents.

So the value is a digest of the payload, not of the manifest or the board. The
DLL contains a textbook reflected CRC-32 table at `0x181d85580`
(entries `0x7c..=0x81` are `0x59b33d17 0x2eb40d81 0xb7bd5c3b 0xc0ba6cad
0xedb88320 0x9abfb3b6`), which is CRC-32/IEEE with polynomial `0xEDB88320`.
CRC-32 is therefore the algorithm the binary demonstrably ships; that
`<checksum>` uses it is strongly implied but not proven against a known
payload (see GAPS).

## 3. Download manifest (JSON)

Firmware packages are located through a small JSON document fetched from a
per-product slug **[P]**:

```json
{
  "firmware-URL": "https://...",
  "firmware-md5": "…32 hex…",
  "os-md5": "…",
  "download-URL": "…",
  "archive.md5": "…"
}
```

* `firmware-URL` / `firmware-md5` are required.
* `os-md5`, `download-URL`, `archive.md5` are optional.
* Slugs observed: `ntusbplus-manifest`, `ntusbmini-manifest`,
  `podmic-usb-manifest`; unknown products fall back to a lower-case hyphenated
  name plus `-manifest`.
* Class names: `ManifestDownloadThread`, `DownloadTask`,
  `FallbackDownloadTask`.

Downloading is out of scope for the reconstruction beyond this format: the
crate includes a plain-HTTP downloader and a directory downloader (offline
mirrors). Archive extraction (`.zip`) is not implemented because it would
require a compression dependency.

## 4. Intel HEX

`FirmwareUpdater` decodes Intel HEX images (used for nRF/QCC firmware).
The decoder's user-visible failures are verbatim strings **[P]**:

```text
Intel HEX File decode aborted; ':' missing in first line.
Unknown Intel HEX record @ line 
Invalid hex character
Checksum error @ line 
```

The parser lives at `FUN_1801148d0`; the checksum-error path references
`0x180116474`. From those messages the implemented record set is the standard
one:

| type | meaning |
|---|---|
| `00` | data |
| `01` | end of file |
| `02` | extended segment address |
| `03` | start segment address |
| `04` | extended linear address |
| `05` | start linear address |

Checksums are the two's-complement of the record bytes; `encode` produces the
same form.

## 5. Update ordering

The Dart statuses are **[P]**: `Idle`, `Downloading`, `Installing`, `Applying`,
`Complete`, `DeviceConnected`.

The native pipeline order below is **[I]** (the stages are named by the Dart
layer and the signals; the transition table itself is reconstructed):

| # | Phase | Reported status | Manager work |
|---|---|---|---|
| 1 | `Checking` | `Idle` | compare versions, apply gate |
| 2 | `Downloading` | `Downloading` | fetch package (MD5) |
| 3 | `Verifying` | `Installing` | per-partition size + CRC-32 |
| 4 | `Preparing` | `Installing` | enter flash mode / bootloader |
| 5 | `Erasing` | `Installing` | erase target partitions |
| 6 | `Writing` | `Installing` | write payload |
| 7 | `Validating` | `Installing` | read back and CRC-32 |
| 8 | `Applying` | `Applying` | reboot into the new image |
| 9 | `Reconnecting` | `Applying` | wait for re-enumeration |
| 10 | `Done` | `Complete` | persist completion |

Progress is monotonic and reported both as whole percent (`void(int)`) and as
`0.0..=1.0` (`void(double)`).

### 5.1 Decision gate

`check_firmware` mirrors `DiagnosticsCheckFirmwareUseCase`:

* manifest must validate and its `board_name` must equal the device's board;
* `forceFirmwareUpdate` ⇒ `Required`, ignoring the version comparison;
* device below a configured minimum ⇒ `Required`;
* device older than the manifest ⇒ `Recommended`;
* same version with reinstall requested ⇒ `Optional`;
* otherwise no update (`UpToDate` / `NewerOnDevice`).

`Required` maps to flash mode `Forced`; the other policies use `Normal`.

## 6. Persistence and analytics

Literal keys in the DLL and the Dart layer **[P]**:

```text
firmware_update_required        firmware_update_started
firmware_update_progress        firmware_update_completed
dismissedFirmwareUpdateTile     hide_apply_changes_dialog
hide_formatting_memory_dialog
```

Analytics event names are the first four keys. `PersistedUpdateState` serialises
to a compact JSON object using these literals plus `device_id` and `status`
(e.g. `{"firmware_update_required":false,…,"status":"Complete"}`).

Other observed strings that frame the feature: `Firmware update flag set.`,
`OTA running.`, `Firmware Updater`, `\\?\hid#`, `USB\VID_19F7&PID_`,
`0HIDClass`. RØDE's USB vendor id is `0x19F7`.

Firmware file-name conventions seen in the artifacts:
`NT-USB_Mini v2.3.6.bin`, `NT-USBplus_v1.0.9.bin`,
`NTHW-50-EU-Release_1.0.6_ENCRYPTED_dfu.bin`.

## 7. The transport seam

Nothing in the artifacts reveals the per-family wire protocol: no HID report
layout, no DFU opcode, no bootloader handshake survives in the stripped image.
The crate therefore defines exactly what the pipeline needs and nothing more:

```rust
pub trait FirmwareTransport {
    type Error: Display;
    fn enter_flash_mode(&mut self, mode: FlashMode) -> Result<(), Self::Error>;
    fn erase(&mut self, partition: &Partition) -> Result<(), Self::Error>;
    fn write(&mut self, partition: &str, offset: u32, data: &[u8]) -> Result<(), Self::Error>;
    fn read_back(&mut self, partition: &str) -> Result<Vec<u8>, Self::Error>;
    fn reboot(&mut self) -> Result<FirmwareRevision, Self::Error>;
    fn wait_for_reconnect(&mut self, timeout_ms: u64) -> Result<FirmwareRevision, Self::Error>;
}
```

`WRITE_CHUNK` is 1 KiB; the real chunk size is transport-specific and not
recoverable.

## 8. CLI mapping

| Command | Recovered feature exercised |
|---|---|
| `manifest parse/fixtures/render` | OTA XML model and both serialisations |
| `hex parse/image` | Intel HEX decoder |
| `verify` | size + CRC-32 verification |
| `md5` / `crc32` | package and partition digests |
| `version compare/code` | ordering and digits-as-hex rule |
| `plan` | diagnostics gate and board dispatch |
| `simulate` | full pipeline over `SimulatedTransport` |
| `devices` | updater hierarchy, board profiles, signals |
| `keys` | persistence/analytics literals and status mapping |
