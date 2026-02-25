use std::io::Write;

use rgb_cli::run_with_args;
use tempfile::NamedTempFile;

#[test]
fn headless_smoke_run_exits_cleanly_with_fixed_frame_limit() {
    let mut rom_file = deterministic_test_rom_file();
    rom_file.flush().expect("flush ROM file");
    let rom_path = rom_file.path().display().to_string();

    for attempt in 1..=2 {
        let result = run_with_args(["rgb_cli", "--quiet", "--frames", "2", &rom_path]);
        assert!(
            result.is_ok(),
            "expected stable clean exit for deterministic headless run (attempt {attempt})"
        );
    }
}

fn deterministic_test_rom_file() -> NamedTempFile {
    let mut bytes = vec![0; 0x8000];
    bytes[0x134..0x139].copy_from_slice(b"SMOKE");
    bytes[0x147] = 0x00;
    bytes[0x148] = 0x00;
    bytes[0x149] = 0x00;

    let mut file = NamedTempFile::new().expect("create temp ROM file");
    file.write_all(&bytes).expect("write ROM bytes");
    file
}
