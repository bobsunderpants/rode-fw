# rode-fw

A dependency-free Rust reconstruction of the firmware-update subsystem in the
shipped RØDE Central application (Windows build, `rode_bridge.dll` + Dart
`app.so`).

The goal is not a byte-for-byte decompilation: the artifacts are stripped of
every internal symbol and the device transports are not statically
recoverable. The goal is a faithful **reimplementation of everything the
artifacts do prove** — the manifest formats, the verification algorithms, the
version rules, the device dispatch table, the update ordering and the
user-visible state machine — with the one unrecoverable layer (the wire
protocol) behind an explicit trait.

Everything here is derived from the shipped binaries with REA; the evidence
trail, down to file offsets, is in [`docs/EVIDENCE.md`](docs/EVIDENCE.md).

## Status

| Area | State | Module | Confidence |
|---|---|---|---|
| OTA manifest XML (both serialisation styles) | implemented, byte-exact round-trip on 12 real manifests | `src/ota_xml.rs` | high |
| Intel HEX decoder/encoder | implemented; DLL error strings reproduced | `src/hex.rs` | high |
| CRC-32/IEEE verification | implemented; table recovered | `src/crc32.rs` | high |
| MD5 package verification | implemented; RFC 1321 vectors | `src/md5.rs` | high |
| `version_code` derivation | implemented; rule holds for all 12 fixtures | `src/version.rs` | high |
| Download manifest JSON | implemented | `src/json.rs`, `src/download.rs` | high |
| Board → family/updater table | implemented, per-board confidence annotated | `src/device.rs` | mixed |
| Update pipeline + state machine | implemented and tested end-to-end | `src/manager.rs`, `src/state.rs` | inferred ordering |
| Persistence/analytics keys | implemented | `src/persistence.rs` | high |
| Byte-level flash/bootloader protocol | **not recoverable**; trait seam | `src/manager.rs` | gap |
| `FirmwareUpdater::State` / `FlashMode` enumerator values | **not recoverable**; documented reconstruction | `src/state.rs` | gap |

## Provenance

| Artifact | SHA-256 | Facts |
|---|---|---|
| `data/app.so` | `76137cc7b7b60d1b32fe05df635a3d5acf0a535f8e8b530dd42ff17db3148ccb` | ELF64 x86-64, stripped, ~11 MB, `image_base 0x100000`, 23,267 strings, Dart AOT (0 procedures) |
| `rode_bridge.dll` | `4b0c7d47c79aee563a66ea088110b91439e078fd055968ee94711856909099c0` | PE32+ x86-64, image base `0x180000000`, `.text` 4.4 MB, `.rdata` 28 MB, 14,074 procedures, 5,099 strings |

Test vectors in `src/fixtures.rs` are the twelve `ota_firmware` documents
embedded in the DLL's `.rdata`; their source addresses are listed in that
file's module documentation.

## Build and test

No dependencies, no network:

```console
$ cargo build --release
$ cargo test
```

64 unit tests plus 3 doc tests. Notably:

* all 12 embedded manifests parse, validate and re-serialise **byte for byte**
  in both `XmlStyle::Tabbed` and `XmlStyle::Compact`;
* CRC-32 matches the table bytes recovered from the DLL;
* MD5 matches the RFC 1321 vectors;
* the full pipeline is exercised against `SimulatedTransport`, including
  checksum failure, missing payload, transport failure and a device that
  re-enumerates with the wrong version.

## CLI

The binary is a way to inspect the evidence and to run the pipeline offline.

```console
$ rode-fw manifest fixtures
board       version     code parts      bytes  style
wgo2_rx        1.93    0x193     2     528104  tabbed
...
wmcam_rx       2.31    0x231     4     764544  compact

$ rode-fw plan --board wgo2_rx --current 1.80
update       1.80 (0x180) -> 2.50 (0x250)
board        wgo2_rx
family       Wireless GO II
updater      WGo2FirmwareUpdater
policy       recommended (newer-available)
flash mode   normal
bytes        700824
partitions   2
  BOOT     fw0_boot        4096B crc32=0x894d9dde
  SYSTEM   fw0_sys       696728B crc32=0x4d051b4f

$ rode-fw simulate --board wgo2_rx --current 1.80 --synth
...
  Installing 92% validating (fw0_sys)
  verified fw0_boot crc32=0xbd77fe18
  verified fw0_sys crc32=0xd77961f3
  Applying 96% applying (fw0_sys)
  Applying 99% reconnecting (fw0_sys)
  Complete 100% done (fw0_sys)
  analytics firmware_update_completed
  persist {"firmware_update_required":false,"firmware_update_started":true,
           "firmware_update_progress":100,"firmware_update_completed":true,...}
done: 1.80 (0x180) -> 2.50 (0x250) (700824 bytes, 2 partitions)
transport    1 flash-mode, 2 erase, 685 write, 2 read-back, 1 reboot, 1 reconnect
```

Other subcommands:

```text
manifest parse <xml|->             summarise an OTA manifest
manifest render <xml|-> [--style]  re-render canonically
hex parse <file>                   summarise Intel HEX segments
hex image <file> [-o out.bin]      decode to a flat image
verify --manifest <xml> --dir <d>  check sizes and CRC-32
md5 <file> | crc32 <file>          compute the two digests
version compare <a> <b>            numeric comparison (1.9 < 1.10)
version code <name>                digits-as-hex derivation
devices | keys                     board, signal and key tables
```

`simulate` needs payload bytes: `--dir` points at a mirrored package,
`--synth` generates deterministic bytes and recomputes the manifest checksums
(clearly a simulation, never a verification).

## Library shape

```rust
use rode_fw::{check_firmware, DeviceInfo, OtaManifest, UpdateGate};

let manifest = OtaManifest::parse(rode_fw::fixtures::WGO2_RX_1_93)?;
let device = DeviceInfo::from_strs("wgo2_rx", "1.80")?;
let decision = check_firmware(&device, &manifest, &UpdateGate::normal())?;
```

`UpdatePlan::execute` drives any `FirmwareTransport` and reports through any
`EventSink`. `SimulatedTransport` and `EventLog` are provided for tests.

The crate builds with `#![forbid(unsafe_code)]` and no `unsafe`, no crates.io
dependencies.

## Layout

```text
src/
  error.rs        error taxonomy (incl. `Invalid Nordic firmware.`)
  crc32.rs        CRC-32/IEEE, DLL table
  md5.rs          MD5
  json.rs         minimal JSON (download manifests, analytics)
  version.rs      version parsing, digits-as-hex code, numeric ordering
  ota_xml.rs      OTA manifest model, parser, serializer, validation
  fixtures.rs     the 12 manifests from the DLL, as test vectors
  hex.rs          Intel HEX decoder/encoder
  device.rs       updater hierarchy, board profiles, product strings
  state.rs        statuses, phases, flash modes, monotonic state machine
  download.rs     download manifest + HTTP/directory/memory downloaders
  persistence.rs  persistence keys, analytics events, persisted state
  manager.rs      planning, payload verification, flash pipeline, transport
  cli.rs          command-line front end
docs/
  PROTOCOL.md     recovered formats and ordering
  EVIDENCE.md     claim → address/string → confidence
  GAPS.md         what is not recoverable and why
```

## Related work

`../pcm-a10-fw/` is an unrelated reimplementation of a different device's
firmware format; this crate only borrows its house style (dependency-free,
lib+bin, docs/ with evidence and gaps).

## License

MIT OR Apache-2.0. No RØDE code or firmware is included; only format facts and
short strings needed for compatibility.
