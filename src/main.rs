//! `rode-fw` binary entry point.
//!
//! All work happens in [`rode_fw::cli`]; this file only collects arguments and
//! maps errors to an exit status.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(message) = rode_fw::cli::run(&args) {
        eprintln!("rode-fw: {message}");
        std::process::exit(1);
    }
}
