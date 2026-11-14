# Gaps and reconstruction boundaries

The artifacts are stripped of internal symbols, and the parts of the update
feature that only exist as machine code operating on device handles cannot be
read out statically. This file lists every place where the crate stops being a
transcription and becomes a reconstruction, what that costs, and what evidence
would close the gap.

Confidence labels match [`EVIDENCE.md`](EVIDENCE.md).

---

## 1. Byte-level device transport (largest gap)

**Status:** not recoverable from the shipped binaries.

**What is missing:** for every family (`WGo2`, `WMe`, `WPro`, `WMicro`,
`FilmmakerII`, `IntPro`, `Default`, `Generic`, `Ota`, `QccHid`) the actual
frames: HID report size, command opcodes, enter-bootloader sequence, erase
granularity, write acknowledgement, read-back command, reboot command and
re-enumeration timing.

**Why:** the DLL contains no HID report layout, no DFU/OTA opcode table and no
named transport procedure. The transport state lives in `Session`,
`HidDevices`, `NthwDeviceIo` and `RodeSettingsHidTransport`, all of which are
only RTTI names.

**What the crate does:** `FirmwareTransport` in `src/manager.rs` defines the
six operations the pipeline needs and nothing else. `SimulatedTransport`
implements them in memory. A real port implements the trait per family; the
rest of the crate does not change.

**How to close:** capture USB traffic (USBPcap/Wireshark) for one update per
family, or instrument `HidDevices`/`NthwDeviceIo` at runtime and log calls.
Vendor DFU documentation would also suffice for the QCC path.

## 2. `FirmwareUpdater::State` and `W4FlashMode` values

**Status:** names, values and enumerator order are absent.

**Evidence:** only the signal type `void(State)` survives in `slot_pmf` RTTI.
No state-name strings (e.g. `Idle`, `Flashing`, `Erasing`) and no RTTI
`enum` metadata appear in `.rdata` or `.data`.

**What the crate does:** `Phase` and `FlashMode` are its own enums, documented
as reconstructions. `FlashMode` has `Normal`, `Forced`, `Recovery`, matching
the three behaviours the Dart use cases expose; `Phase` is the ordered pipeline
in `PROTOCOL.md` §5.

**How to close:** runtime logging of the `void(State)` signal values while
exercising normal, forced and interrupted updates, or any surviving `.pdb`.

## 3. `FlashMode` recovery path

**Status:** inferred.

`forceFirmwareUpdate` proves a forced path. A third "device is sitting in
bootloader after an interrupted update" behaviour is strongly suggested by the
`DeviceConnected`/reconnect states and the `Firmware update flag set.` /
`OTA running.` strings, but no separate enumerator was recovered.

## 4. Nordic image validation beyond structure

**Status:** structural checks only.

**Implemented:** for Nordic boards the manifest must contain `BOOT` +
`SYSTEM` partitions; payloads must match declared size and CRC-32.

**Not implemented:** validation of the nRF vector table, SoftDevice/base
address ranges, DFU init packet or signature. The Dart error
`Invalid Nordic firmware.` / `invalidNordicFw` proves such a validation exists
but not what it checks.

**Why:** no firmware payloads ship with the artifacts; only manifests.

**How to close:** one genuine `mbrec.bin` + `zephyr.bin` pair per board, then
diff the validator's accept/reject behaviour under instrumentation.

## 5. `<checksum>` algorithm

**Status:** CRC-32/IEEE is inferred, not confirmed on a payload.

**What is proven:** the value is content-derived (`tone.bin` and `mbrec.bin`
have identical checksums across four boards), and the DLL contains a textbook
reflected CRC-32 table at `0x181d85580`.

**Residual risk:** the manifest could use a different 32-bit digest whose table
coincidentally exists for another purpose. Low, but not zero.

**How to close:** one shipped firmware file plus its manifest entry; compute
CRC-32 and compare.

## 6. Firmware package archive format

**Status:** not implemented.

`archive.md5` and `download-URL` imply a container (likely a ZIP) around the
partition files. The crate reads payloads from a directory or memory and does
not unpack containers: doing so without a compression dependency would be
incomplete, and adding one would break the dependency-free rule.

**How to close:** identify the container magic from one real download, then
either add a vetted dependency or implement the specific deflate subset.

## 7. HTTPS

**Status:** not implemented.

`HttpFileDownloader` speaks plain HTTP/1.1 only. Implementing TLS without a
dependency would not honestly validate certificates, so the crate refuses
rather than pretends. `DirectoryDownloader` and the `Downloader` trait cover
offline and platform-TLS integrations.

## 8. Download-manifest location and retry policy

**Status:** partially known.

Slugs and JSON keys are proven; the base URL, request headers, retry/backoff
policy of `ManifestDownloadThread` / `FallbackDownloadTask`, and how
`download-URL` relates to `firmware-URL` are not.

**What the crate does:** `manifest_slug` reproduces the observed names and the
lower-case fallback; the downloader performs a single `GET` with no retry.

## 9. Board name ↔ product mapping

**Status:** mixed confidence, annotated per row.

`<board_name>` values are proven. Which physical product reports which board is
not: `getConnectedDevices` returns a product/board string whose derivation is
in the Dart layer (no procedures). The product strings and codenames
(`BettyRx`/`BettyTx`, `WilmaFp`/`WilmaPp`, `WIGO2`, `Wireless_GO`,
`Wireless_PRO`, `XDM100`) are proven to exist, but their exact mapping to
`wgo2_*`, `wgo25_*`, `wme_*`, `wm_*`, `wmcam_*` is not.

**How to close:** enumerate USB descriptors for each product and log the
`board_name` reported to the update check.

## 10. `wgo25_*` and the `wm_*` updater

**Status:** unresolved naming, low confidence.

`wgo25_rx`/`wgo25_tx` ship distinct images at the same 2.50 version and the
same Nordic partition layout as `wgo2_*`. No symbol explains the `25` suffix;
`WGo3FirmwareUpdater` is a candidate but unproven, so the table keeps them on
`WGo2` with `Confidence::Low`.

`wm_*`/`wmcam_*` have no class whose name matches the prefix. They are assigned
`DefaultFirmwareUpdater` with `Confidence::Low` purely because they do not fit
any wireless or QCC class.

## 11. Progress percentages and stage weights

**Status:** reconstruction.

The signal *types* are proven; the real percentage at which each stage starts
and ends is not. The crate uses fixed bands (verify 2 %, prepare 5 %, erase
10–20 %, write 20–90 %, validate 92 %, apply 96 %, reconnect 99 %, done 100 %)
and guarantees monotonicity.

## 12. Reconnect semantics

**Status:** reconstruction.

`DeviceConnected` is a proven status. The timeout (30 s), what the native layer
polls (HID path, VID/PID, serial), and whether it accepts a different board id
after a paired-device update are unknown. The crate waits a fixed timeout and
requires the same `board_name` and version code; anything else is a transport
error.

## 13. Persistence medium and format

**Status:** keys proven, storage not.

The seven literal keys are proven. Whether they are written by Dart shared
preferences, a native settings file or the registry is not; the JSON shape in
`src/persistence.rs` is the crate's own serialisation of the same facts.

## 14. Error propagation

**Status:** partial.

User-visible strings are known (`Invalid Nordic firmware.`, `Checksum error @
line `, `Firmware update flag set.`, `OTA running.`). How native failures are
mapped onto Dart exceptions, and which failures are retried, is not.

## 15. Dart orchestration internals

**Status:** names only.

`app.so` yields zero procedures. Use-case names, statuses, keys and bridge
method names are recovered; the control flow inside them (when the tile is
dismissed, what `SessionInfo` carries, how local-binary updates differ from
downloads) is not. The crate models the externally visible decision gate only.

## 16. Signal ordering and threading

**Status:** partial.

Which classes instantiate which signals is proven. The emission order between
`void(int)`, `void(bool)`, `void(State)` and `void(double)`, and whether they
fire on the UI thread, is not. `UpdateEvent` emits progress/analytics in
pipeline order on the caller's thread.

---

## Non-goals

* The device UI, audio routing, recording and formatting features are out of
  scope; `hide_apply_changes_dialog` / `hide_formatting_memory_dialog` are
  documented only because they share the update tile's persistence namespace.
* No RØDE firmware or application code is copied; only format facts and the
  short user-visible strings needed for compatibility.
