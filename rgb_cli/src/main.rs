fn main() {
    if let Err(error) = rgb_cli::run() {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}
