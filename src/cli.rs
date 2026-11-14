//! Command-line front end.
//!
//! Every subcommand maps onto one recovered piece of the subsystem, so the
//! binary doubles as a way to inspect the evidence:
//!
//! ```text
//! rode-fw manifest parse <xml>          # OTA manifest reader
//! rode-fw manifest fixtures            # the 12 manifests embedded in the DLL
//! rode-fw manifest render <xml> --style tabbed|compact
//! rode-fw hex parse <hex>              # Intel HEX decoder
//! rode-fw hex image <hex> -o out.bin    # decode to a flat image
//! rode-fw verify --manifest <xml> --dir <pkg>
//! rode-fw md5 <file> | crc32 <file>
//! rode-fw version compare <a> <b> | version code <name>
//! rode-fw plan --board <id> --current <ver> [--manifest <xml>]
//! rode-fw simulate --board <id> --current <ver> [--dir <pkg> | --synth]
//! rode-fw devices
//! rode-fw keys
//! ```

use std::fmt::Write as _;

use crate::crc32;
use crate::device::{self, UpdaterKind, BOARD_PROFILES};
use crate::fixtures;
use crate::hex::HexImage;
use crate::manager::{
    check_firmware, verify_payloads, DeviceInfo, EventLog, FirmwareRevision, PlanDecision,
    SimulatedTransport, UpdateGate, UpdatePlan,
};
use crate::md5;
use crate::ota_xml::{OtaManifest, XmlStyle};
use crate::persistence::{AnalyticsEvent, PersistenceKey};
use crate::state::{FirmwareUpdateStatus, FlashMode, Phase};
use crate::version::FwVersion;

/// `outln!` that tolerates a closed pipe (`rode-fw ... | head`).
macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}

/// `print!` that tolerates a closed pipe.
macro_rules! out {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = write!(std::io::stdout(), $($arg)*);
    }};
}

/// Runs the CLI with arguments **excluding** the program name.
///
/// Returns a human-readable message on failure; `main` prints it to stderr and
/// exits non-zero.
pub fn run(args: &[String]) -> std::result::Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    let rest = &args[1..];
    match command {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "manifest" => manifest(rest),
        "hex" => hex(rest),
        "verify" => verify(rest),
        "md5" => hash(rest, Hash::Md5),
        "crc32" => hash(rest, Hash::Crc32),
        "version" => version(rest),
        "plan" => plan(rest),
        "simulate" | "flash" => simulate(rest),
        "devices" => devices(),
        "keys" => keys(),
        other => Err(format!("unknown command {other:?}; try `rode-fw help`")),
    }
}

fn print_help() {
    outln!(
        "\
rode-fw — RØDE Central firmware subsystem, rebuilt

USAGE:
  rode-fw <command> [options]

COMMANDS:
  manifest parse <xml|->             summarise an OTA manifest
  manifest fixtures                  list the 12 embedded manifests
  manifest render <xml|-> [--style tabbed|compact]
                                     re-render a manifest (canonical form)
  hex parse <file>                   summarise an Intel HEX file
  hex image <file> [-o out.bin]      decode Intel HEX to a flat image
  verify --manifest <xml> --dir <d>  check payload sizes + CRC-32
  md5 <file>                         MD5 of a file (as `firmware-md5`)
  crc32 <file>                       CRC-32/IEEE of a file (as `<checksum>`)
  version compare <a> <b>            numeric version comparison
  version code <name>                digits-as-hex `version_code` derivation
  plan --board <id> --current <ver> [--manifest <xml>] [--forced]
       [--min <ver>] [--reinstall]   decide whether to update
  simulate --board <id> --current <ver> [--manifest <xml>]
       [--dir <pkg> | --synth] [--forced] [--min <ver>] [--reinstall]
       [--verbose]
                                     run the full pipeline against an
                                     in-memory device
  devices                            board -> family/updater table
  keys                               persistence, analytics and signal tables

The `simulate` pipeline reproduces the recovered ordering:
  verify -> prepare -> erase -> write -> validate -> apply -> reconnect
The byte-level transport is simulated; see docs/GAPS.md."
    );
}

// ---------------------------------------------------------------- arguments

fn has(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn value(args: &[String], flag: &str) -> Option<String> {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == flag {
            return it.next().cloned();
        }
        if let Some(rest) = arg.strip_prefix(&format!("{flag}=")) {
            return Some(rest.to_string());
        }
    }
    None
}

fn required(args: &[String], flag: &str) -> std::result::Result<String, String> {
    value(args, flag).ok_or_else(|| format!("missing required option {flag}"))
}

fn positional(args: &[String]) -> Option<&str> {
    let mut skip_value = false;
    for arg in args {
        if skip_value {
            skip_value = false;
            continue;
        }
        if arg.starts_with('-') {
            skip_value = matches!(
                arg.as_str(),
                "--manifest"
                    | "--dir"
                    | "--board"
                    | "--current"
                    | "--min"
                    | "--style"
                    | "--fill"
                    | "-o"
                    | "--output"
            );
            continue;
        }
        return Some(arg.as_str());
    }
    None
}

fn read_text(path: &str) -> std::result::Result<String, String> {
    if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("stdin: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
    }
}

fn load_manifest(path: &str) -> std::result::Result<OtaManifest, String> {
    let text = read_text(path)?;
    OtaManifest::parse(&text).map_err(|e| format!("{path}: {e}"))
}

/// Picks the newest embedded manifest for a board.
fn fixture_for(board: &str) -> std::result::Result<OtaManifest, String> {
    let mut best: Option<OtaManifest> = None;
    for (b, xml) in fixtures::ALL {
        if *b != board {
            continue;
        }
        let manifest = OtaManifest::parse(xml).map_err(|e| format!("fixture {board}: {e}"))?;
        if best
            .as_ref()
            .map(|current| manifest.version() > current.version())
            .unwrap_or(true)
        {
            best = Some(manifest);
        }
    }
    best.ok_or_else(|| format!("no embedded manifest for board {board:?}; pass --manifest"))
}

fn manifest_for(args: &[String], board: &str) -> std::result::Result<OtaManifest, String> {
    match value(args, "--manifest") {
        Some(path) => load_manifest(&path),
        None => fixture_for(board),
    }
}

fn gate_from(args: &[String]) -> std::result::Result<UpdateGate, String> {
    let mut gate = UpdateGate {
        forced: has(args, "--forced"),
        allow_reinstall: has(args, "--reinstall"),
        ..UpdateGate::default()
    };
    if let Some(minimum) = value(args, "--min") {
        gate.minimum = Some(FwVersion::from_name(&minimum).map_err(|e| e.to_string())?);
    }
    Ok(gate)
}

// ---------------------------------------------------------------- manifest

fn manifest(args: &[String]) -> std::result::Result<(), String> {
    match args.first().map(String::as_str) {
        Some("parse") => {
            let path = positional(&args[1..]).ok_or("usage: rode-fw manifest parse <xml|->")?;
            let m = load_manifest(path)?;
            print_manifest(&m);
            Ok(())
        }
        Some("fixtures") => {
            outln!(
                "{:<10} {:>8} {:>8} {:>5} {:>10}  style",
                "board",
                "version",
                "code",
                "parts",
                "bytes"
            );
            for (board, xml) in fixtures::ALL {
                let m = OtaManifest::parse(xml).map_err(|e| format!("fixture {board}: {e}"))?;
                let style = if m.to_xml(XmlStyle::Compact) == *xml {
                    "compact"
                } else {
                    "tabbed"
                };
                outln!(
                    "{:<10} {:>8} {:>8} {:>5} {:>10}  {}",
                    m.board_name(),
                    m.version().name,
                    format!("0x{:x}", m.version().code),
                    m.partitions.len(),
                    m.total_size(),
                    style
                );
            }
            Ok(())
        }
        Some("render") => {
            let path = positional(&args[1..]).ok_or("usage: rode-fw manifest render <xml|->")?;
            let m = load_manifest(path)?;
            let style = match value(args, "--style").as_deref() {
                Some("compact") => XmlStyle::Compact,
                Some("tabbed") | None => XmlStyle::Tabbed,
                Some(other) => return Err(format!("unknown --style {other:?}")),
            };
            out!("{}", m.to_xml(style));
            Ok(())
        }
        _ => Err("usage: rode-fw manifest <parse|fixtures|render>".into()),
    }
}

fn print_manifest(m: &OtaManifest) {
    outln!("board        {}", m.board_name());
    outln!("version      {}", m.version().name);
    outln!("version_code 0x{:x}", m.version().code);
    outln!(
        "code rule    {}",
        if m.code_derivation_consistent() {
            "consistent (digits read as hex)"
        } else {
            "MISMATCH"
        }
    );
    if let Some(res) = &m.version.version_res {
        outln!("version_res  {res}");
    }
    outln!(
        "partitions   {} declared, {} present, {} bytes",
        m.partitions_num,
        m.partitions.len(),
        m.total_size()
    );
    if let Some(p) = device::board_profile(m.board_name()) {
        outln!(
            "family       {} ({} updater, {} confidence)",
            p.family.label(),
            p.updater.class_name(),
            p.confidence
        );
    } else {
        outln!("family       unknown board (not in the embedded set)");
    }
    outln!();
    outln!(
        "{:>2}  {:<9} {:<10} {:>7}  {:<14} {:>8} {:>10} {:>12}",
        "#",
        "type",
        "name",
        "file_id",
        "file_name",
        "storage",
        "size",
        "checksum"
    );
    for (i, p) in m.partitions.iter().enumerate() {
        outln!(
            "{:>2}  {:<9} {:<10} {:>7}  {:<14} {:>8} {:>10} 0x{:08x}",
            i + 1,
            p.ptype.as_str(),
            p.name,
            p.file_id,
            p.file_name,
            p.storage_id
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".into()),
            p.file_size,
            p.checksum
        );
    }
}

// ---------------------------------------------------------------- hex

fn hex(args: &[String]) -> std::result::Result<(), String> {
    let sub = args
        .first()
        .map(String::as_str)
        .ok_or("usage: rode-fw hex <parse|image> <file>")?;
    let rest = &args[1..];
    let path = positional(rest).ok_or("missing Intel HEX file")?;
    let text = read_text(path)?;
    let image = HexImage::parse(&text).map_err(|e| format!("{path}: {e}"))?;
    match sub {
        "parse" => {
            outln!("segments     {}", image.segments.len());
            outln!("bytes        {}", image.total_bytes());
            outln!("eof marker   {}", image.saw_eof);
            match (image.min_address(), image.max_address()) {
                (Some(min), Some(max)) => outln!("range        0x{min:08x}..0x{max:08x}"),
                _ => outln!("range        empty"),
            }
            if let Some(entry) = image.entry_point {
                outln!("entry        0x{entry:08x}");
            }
            outln!("crc32        0x{:08x}", image.crc32());
            outln!();
            outln!("{:>10} {:>10} {:>8}", "address", "end", "bytes");
            for segment in &image.segments {
                outln!(
                    "{:>10} {:>10} {:>8}",
                    format!("0x{:08x}", segment.address),
                    format!("0x{:x}", segment.end()),
                    segment.data.len()
                );
            }
            Ok(())
        }
        "image" => {
            let fill = match value(rest, "--fill") {
                Some(text) => parse_u8(&text)?,
                None => 0xff,
            };
            let bytes = image.to_binary(fill);
            match value(rest, "-o").or_else(|| value(rest, "--output")) {
                Some(out) => {
                    std::fs::write(&out, &bytes).map_err(|e| format!("{out}: {e}"))?;
                    outln!("wrote {} bytes to {out}", bytes.len());
                }
                None => {
                    use std::io::Write;
                    std::io::stdout()
                        .write_all(&bytes)
                        .map_err(|e| format!("stdout: {e}"))?;
                }
            }
            Ok(())
        }
        other => Err(format!("unknown hex subcommand {other:?}")),
    }
}

fn parse_u8(text: &str) -> std::result::Result<u8, String> {
    let text = text.trim();
    let parsed = if let Some(hex) = text.strip_prefix("0x") {
        u8::from_str_radix(hex, 16)
    } else {
        text.parse::<u8>()
    };
    parsed.map_err(|_| format!("not a byte: {text:?}"))
}

// ---------------------------------------------------------------- verify

fn verify(args: &[String]) -> std::result::Result<(), String> {
    let manifest_path = required(args, "--manifest")?;
    let dir = required(args, "--dir")?;
    let m = load_manifest(&manifest_path)?;
    let payloads = crate::manager::PayloadSet::from_dir(&dir).map_err(|e| e.to_string())?;
    verify_payloads(&m, &payloads).map_err(|e| e.to_string())?;
    outln!(
        "OK {} partitions, {} payload files, {} bytes",
        m.partitions.len(),
        payloads.len(),
        payloads.total_bytes()
    );
    for p in &m.partitions {
        outln!(
            "  {:<10} {:<14} {:>9}B crc32=0x{:08x}",
            p.name,
            p.file_name,
            p.file_size,
            p.checksum
        );
    }
    Ok(())
}

// ---------------------------------------------------------------- hashes

enum Hash {
    Md5,
    Crc32,
}

fn hash(args: &[String], which: Hash) -> std::result::Result<(), String> {
    let path = positional(args).ok_or("missing file")?;
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    match which {
        Hash::Md5 => outln!("{}", md5::hex_digest(&bytes)),
        Hash::Crc32 => outln!("0x{:08x}", crc32::crc32(&bytes)),
    }
    Ok(())
}

// ---------------------------------------------------------------- version

fn version(args: &[String]) -> std::result::Result<(), String> {
    match args.first().map(String::as_str) {
        Some("compare") => {
            let a = args
                .get(1)
                .ok_or("usage: rode-fw version compare <a> <b>")?;
            let b = args
                .get(2)
                .ok_or("usage: rode-fw version compare <a> <b>")?;
            let va = FwVersion::from_name(a).map_err(|e| e.to_string())?;
            let vb = FwVersion::from_name(b).map_err(|e| e.to_string())?;
            let relation = match va.cmp(&vb) {
                std::cmp::Ordering::Less => "less",
                std::cmp::Ordering::Equal => "equal",
                std::cmp::Ordering::Greater => "greater",
            };
            outln!("{a} ({:#x}) {relation} {b} ({:#x})", va.code, vb.code);
            Ok(())
        }
        Some("code") => {
            let name = args.get(1).ok_or("usage: rode-fw version code <name>")?;
            let code = FwVersion::code_from_name(name).map_err(|e| e.to_string())?;
            outln!("0x{code:x}");
            Ok(())
        }
        _ => Err("usage: rode-fw version <compare|code>".into()),
    }
}

// ---------------------------------------------------------------- plan

fn plan(args: &[String]) -> std::result::Result<(), String> {
    let board = required(args, "--board")?;
    let current = required(args, "--current")?;
    let device = DeviceInfo::from_strs(&board, &current).map_err(|e| e.to_string())?;
    let m = manifest_for(args, &board)?;
    let gate = gate_from(args)?;
    match check_firmware(&device, &m, &gate).map_err(|e| e.to_string())? {
        PlanDecision::UpToDate {
            current,
            latest,
            reason,
        } => {
            outln!("no update: device {current} vs manifest {latest} ({reason})");
        }
        PlanDecision::Update(plan) => print_plan(&plan),
    }
    Ok(())
}

fn print_plan(plan: &UpdatePlan) {
    outln!("update       {} -> {}", plan.from, plan.to);
    outln!("board        {}", plan.board);
    outln!("family       {}", plan.family.label());
    outln!("updater      {}", plan.updater.class_name());
    outln!("policy       {} ({})", plan.policy, plan.reason);
    outln!("flash mode   {}", plan.flash_mode());
    outln!("bytes        {}", plan.total_bytes);
    outln!("partitions   {}", plan.manifest.partitions.len());
    for p in &plan.manifest.partitions {
        outln!(
            "  {:<9} {:<10} {:>9}B crc32=0x{:08x}",
            p.ptype.as_str(),
            p.name,
            p.file_size,
            p.checksum
        );
    }
}

// ---------------------------------------------------------------- simulate

fn simulate(args: &[String]) -> std::result::Result<(), String> {
    let board = required(args, "--board")?;
    let current = required(args, "--current")?;
    let device = DeviceInfo::from_strs(&board, &current).map_err(|e| e.to_string())?;
    let mut m = manifest_for(args, &board)?;
    let gate = gate_from(args)?;

    let mut payloads = if let Some(dir) = value(args, "--dir") {
        crate::manager::PayloadSet::from_dir(&dir).map_err(|e| e.to_string())?
    } else if has(args, "--synth") {
        let set = synthetic_payloads(&m);
        refresh_checksums(&mut m, &set);
        set
    } else {
        return Err(
            "simulate needs payloads: pass --dir <package dir> or --synth for generated bytes"
                .into(),
        );
    };
    payloads.device_id = Some(device.id());

    match check_firmware(&device, &m, &gate).map_err(|e| e.to_string())? {
        PlanDecision::UpToDate {
            current,
            latest,
            reason,
        } => {
            outln!("no update: device {current}, manifest {latest} ({reason})");
            Ok(())
        }
        PlanDecision::Update(plan) => {
            print_plan(&plan);
            outln!();
            let mut transport = SimulatedTransport::new(plan.board.clone(), plan.from.clone());
            transport.set_revision(FirmwareRevision::new(plan.board.clone(), plan.to.clone()));
            let mut sink = EventLog::new();
            match plan.execute(&payloads, &mut transport, &mut sink) {
                Ok(outcome) => {
                    for event in sink.events() {
                        outln!("  {}", event.describe());
                    }
                    outln!();
                    outln!(
                        "done: {} -> {} ({} bytes, {} partitions)",
                        outcome.from,
                        outcome.to,
                        outcome.bytes_written,
                        outcome.partitions.len()
                    );
                    if has(args, "--verbose") {
                        outln!("transport ops:");
                        for line in transport.log_lines() {
                            outln!("  {line}");
                        }
                    } else {
                        outln!("transport    {}", transport_summary(&transport));
                    }
                    Ok(())
                }
                Err(err) => {
                    for event in sink.events() {
                        outln!("  {}", event.describe());
                    }
                    Err(err.to_string())
                }
            }
        }
    }
}

fn transport_summary(transport: &SimulatedTransport) -> String {
    use crate::manager::SimulatedOp;
    let mut modes = 0usize;
    let mut erases = 0usize;
    let mut writes = 0usize;
    let mut reads = 0usize;
    let mut reboots = 0usize;
    let mut reconnects = 0usize;
    for op in transport.log() {
        match op {
            SimulatedOp::EnterFlashMode(_) => modes += 1,
            SimulatedOp::Erase(_) => erases += 1,
            SimulatedOp::Write { .. } => writes += 1,
            SimulatedOp::ReadBack(_) => reads += 1,
            SimulatedOp::Reboot => reboots += 1,
            SimulatedOp::WaitForReconnect => reconnects += 1,
        }
    }
    format!(
        "{modes} flash-mode, {erases} erase, {writes} write, {reads} read-back, {reboots} reboot, {reconnects} reconnect"
    )
}

fn synthetic_payloads(m: &OtaManifest) -> crate::manager::PayloadSet {
    let mut set = crate::manager::PayloadSet::new();
    for p in &m.partitions {
        let bytes: Vec<u8> = (0..p.file_size)
            .map(|i| (i.wrapping_mul(31) ^ 0x5a) as u8)
            .collect();
        set.insert(p.file_name.clone(), bytes);
    }
    set
}

fn refresh_checksums(m: &mut OtaManifest, payloads: &crate::manager::PayloadSet) {
    for p in &mut m.partitions {
        if let Some(bytes) = payloads.get(&p.file_name) {
            p.file_size = bytes.len() as u32;
            p.checksum = crc32::crc32(bytes);
        }
    }
}

// ---------------------------------------------------------------- tables

fn devices() -> std::result::Result<(), String> {
    outln!(
        "{:<10} {:<26} {:<28} {:<10} signals",
        "board",
        "family",
        "updater",
        "confidence"
    );
    for p in BOARD_PROFILES {
        let s = p.updater.signals();
        let mut flags = String::new();
        if s.progress {
            flags.push_str("progress ");
        }
        if s.completion {
            flags.push_str("completion ");
        }
        if s.done {
            flags.push_str("done ");
        }
        outln!(
            "{:<10} {:<26} {:<28} {:<10} {}",
            p.board,
            p.family.label(),
            p.updater.class_name(),
            p.confidence,
            flags.trim_end()
        );
    }
    Ok(())
}

fn keys() -> std::result::Result<(), String> {
    outln!("persistence keys");
    for key in PersistenceKey::ALL {
        outln!("  {}", key.as_str());
    }
    outln!();
    outln!("analytics events");
    for event in [
        AnalyticsEvent::Required,
        AnalyticsEvent::Started,
        AnalyticsEvent::Progress,
        AnalyticsEvent::Completed,
    ] {
        outln!("  {}", event.as_str());
    }
    outln!();
    outln!("dart statuses");
    for status in FirmwareUpdateStatus::ALL {
        outln!(
            "  {:<16} {}",
            status.as_str(),
            Phase::ALL
                .iter()
                .filter(|p| p.status() == status)
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    outln!();
    outln!("flash modes");
    for mode in FlashMode::ALL {
        outln!("  {}", mode);
    }
    outln!();
    outln!("updater classes");
    for kind in UpdaterKind::ALL {
        let s = kind.signals();
        let mut flags = String::new();
        if s.progress {
            let _ = write!(flags, "progress ");
        }
        if s.completion {
            let _ = write!(flags, "completion ");
        }
        if s.done {
            let _ = write!(flags, "done ");
        }
        outln!(
            "  {:<28} {:<12} {}",
            kind.class_name(),
            if kind.is_wireless() { "wireless" } else { "" },
            flags.trim_end()
        );
    }
    Ok(())
}
