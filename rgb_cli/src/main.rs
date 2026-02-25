//! Thin binary entrypoint for `rgb_cli`.
//!
//! Responsibility boundary:
//! - delegate all behavior to `rgb_cli::run()`
//! - print user-facing errors to stderr
//! - map `CliError` to stable process exit codes

fn main() {
    if let Err(error) = rgb_cli::run() {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}
