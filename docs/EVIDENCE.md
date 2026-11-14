# Evidence

Everything implemented in this crate is traceable to bytes in one of two
shipped artifacts. This file records **what was observed**, **where**, and
**how strongly** it supports the code.

## Artifacts

| Artifact | SHA-256 | Static facts |
|---|---|---|
| `data/app.so` | `76137cc7b7b60d1b32fe05df635a3d5acf0a535f8e8b530dd42ff17db3148ccb` | ELF64 x86-64, stripped, ≈11 MB, `image_base 0x100000`, 23,267 strings, Dart AOT snapshot (no native procedures) |
| `rode_bridge.dll` | `4b0c7d47c79aee563a66ea088110b91439e078fd055968ee94711856909099c0` | PE32+ x86-64, `image_base 0x180000000`, `.text` ≈4.4 MB, `.rdata` ≈28 MB, 14,074 procedures, 5,099 strings |

Tooling: REA (Ghidra 12.2 backend), read-only static analysis; no execution of
either artifact.

## Method

1. `app.so` was opened first to recover the Dart-side domain model: a
   `firmware` string inventory (185 rows) yielded the use-case, cubit, route,
   key and status names quoted in `PROTOCOL.md` §1.
2. `rode_bridge.dll` was then opened and used for all format, algorithm and
   hierarchy facts. Class names come from MSVC RTTI `TypeDescriptor` strings,
   signal signatures from `slot_pmf` descriptors, and algorithm constants from
   data segments.
3. The twelve `ota_firmware` documents were read byte-for-byte out of
   `.rdata` and are reproduced verbatim in `src/fixtures.rs`.

### Negative results (important)

* `app.so` reports **zero procedures**. Its exported Dart ABI entry points are
  tail-call thunks through `DAT_181f29868 + 0x2c0`; they do not lead statically
  into firmware code. This is why the Dart layer is used as an orchestration
  oracle only.
* A full byte search of all initialized segments for the RTTI
  `CompleteObjectLocator` patterns (`18f01e01`, `70e01e01`, and relatives)
  returned **no matches**, despite thousands of `TypeDescriptor` hits. The
  vtables are therefore not RTTI-linked, so virtual function tables cannot be
  walked to recover per-class methods.
* `FirmwareUpdater::State` and `W4FlashMode` enumerator **names and values are
  absent** from the image; only the signal signature `void(State)` survives.
  The enums in `src/state.rs` are explicitly a reconstruction.
* No HID report layout, DFU opcode, erase command or bootloader handshake is
  present as data or as a named procedure. That layer is a documented gap.

## Claim → evidence

| Claim | Evidence | Confidence |
|---|---|---|
| CRC-32/IEEE | Reflected table at `0x181d85580`; words at `0x7c..=0x81` are `0x59b33d17 0x2eb40d81 0xb7bd5c3b 0xc0ba6cad 0xedb88320 0x9abfb3b6` | proven |
| `<checksum>` is content-derived | `tone.bin` = `0xd81bef6b` and `mbrec.bin` = `0xe4d27a38` across all four `wm_*` manifests | proven (value), inferred (algorithm) |
| MD5 is linked | First sine-table constant `0xd76aa478` at `0x1801d48a0`; `firmware-md5`/`archive.md5` keys; RFC 1321 vectors pass in `src/md5.rs` | proven |
| Intel HEX decoder and errors | Verbatim `.rdata` strings `Intel HEX File decode aborted; ':' missing in first line.`, `Unknown Intel HEX record @ line `, `Invalid hex character`, `Checksum error @ line `; parser `FUN_1801148d0`; checksum path at `0x180116474` | proven |
| OTA manifest schema | Twelve embedded documents (addresses below), parsed and re-serialised byte-for-byte | proven |
| `version_code` rule | Digits of `version_name` read as hex; holds 12/12 (`1.93→0x193`, `2.50→0x250`, `1.47→0x147`, `2.31→0x231`) | proven |
| Partition catalogues | Nordic: `BOOT fw0_boot` id 1 `mbrec.bin` + `SYSTEM fw0_sys` id 3 `zephyr.bin`; `wm_*`: `SYSTEM` id 4 `app.bin`, `DATA` id 39 `tone.bin`, `BOOT` id 1 `mbrec.bin`, `SYS_PARAM` id 2 `param.bin`, all `storage_id 0` | proven |
| Two XML styles | Eight tabbed documents and four compact documents differ in child order, indentation and `version_res`; both re-serialised exactly | proven |
| Updater class hierarchy | RTTI type descriptors: `FirmwareUpdater`; `IntPro`, `Wireless`, `WGo2`, `WGo3`, `WMe`, `WPro`, `WMicro`, `FilmmakerII`, `Default`, `Generic`, `Ota`, `QccHid` `FirmwareUpdater`; plus `FirmwareUpdateManager`, `FirmwareDataOTA`, `Session`, `ManifestDownloadThread`, `DownloadTask`, `FallbackDownloadTask`, `HidDevices`, `NthwDeviceIo`, `RodeSettingsHidTransport` | proven (names) |
| Signal signatures | `slot_pmf` RTTI: `void(int)`, `void(bool)`, `void()`, `void(State)`, `void(double)` | proven |
| Board → updater mapping | Prefix match (`wgo2_*` ↔ `WGo2FirmwareUpdater`, `wme_*` ↔ `WMeFirmwareUpdater`); `wm_*`/`wgo25_*` have no matching class name | high for prefix pairs, low for the rest (annotated per row in `src/device.rs`) |
| Update pipeline ordering | Dart statuses `Idle/Downloading/Installing/Applying/Complete/DeviceConnected`, use-case names, manager signals | inferred ordering |
| Persistence/analytics keys | Literal `.rdata` strings: `firmware_update_required`, `firmware_update_started`, `firmware_update_progress`, `firmware_update_completed`, `dismissedFirmwareUpdateTile`, `hide_apply_changes_dialog`, `hide_formatting_memory_dialog` | proven |
| Download manifest keys | `firmware-URL`, `firmware-md5`, `os-md5`, `download-URL`, `archive.md5`; slugs `ntusbplus-manifest`, `ntusbmini-manifest`, `podmic-usb-manifest` | proven |
| USB identity | `VID_19F7`, `USB\VID_19F7&PID_`, `\\?\hid#`, `0HIDClass` | proven |
| Device codenames | `BettyRx`/`BettyTx`, `WilmaFp`/`WilmaPp`, `WIGO2`, `Wireless_GO`, `Wireless_PRO`, `XDM100` | proven |
| `FlashMode` variants | Not present; three behaviours inferred from normal/forced/recovery use cases | reconstruction |
| Transport protocols | No data or named procedures | gap |

## Embedded manifest addresses

Image base `0x180000000`; the same table is in `src/fixtures.rs`.

| board | version | address |
|---|---|---|
| `wgo2_rx` | 1.93 | `0x18068bd94` |
| `wgo2_tx` | 1.93 | `0x18070d5c6` |
| `wgo2_rx` | 2.50 | `0x1807b26b1` |
| `wgo2_tx` | 2.50 | `0x18085e0eb` |
| `wgo25_rx` | 2.50 | `0x18090c9b2` |
| `wgo25_tx` | 2.50 | `0x1809b77ed` |
| `wme_rx` | 1.47 | `0x181056eb0` |
| `wme_tx` | 1.47 | `0x1810b40e9` |
| `wm_tx` | 2.31 | `0x1815e7d8d` |
| `wm_rx` | 2.31 | `0x1816a19ca` |
| `wm_crx` | 2.31 | `0x181753208` |
| `wmcam_rx` | 2.31 | `0x181808648` |

## Reproducing the key observations

With REA bound to `rode_bridge.dll`:

```text
search_strings  pattern="FirmwareUpdater"          # RTTI class names
search_strings  pattern="firmware_update_"         # persistence/analytics keys
search_strings  pattern="Intel HEX"                # decoder failures
search_strings  pattern="firmware-md5"             # download manifest keys
search_bytes    pattern="59b33d172eb40d81b7bd5c3b" # CRC table
search_bytes    pattern="78a46ad7"                 # MD5 sine constant (LE)
analyze_function procedure="0x1801148d0"           # Intel HEX decoder
list_strings / inspect_analysis_view               # 12 manifest documents
```

`list_names` over the whole DLL exceeds the tool response budget; page through
`inspect_analysis_view` instead.

## Caveats

* The live REA investigation records were not exported before the analysis
  session closed, so this file is the evidence record. Both artifact hashes are
  included so any claim can be re-derived.
* RTTI proves class **names**, not class **methods**. Statements about which
  updater handles which board are marked accordingly in `src/device.rs` and
  `GAPS.md`.
* CRC-32 is the algorithm the binary ships; that the manifest `<checksum>` uses
  it is inferred from content-derivation plus the presence of the table, not
  from a known payload pair.
