use rgb_core::gameboy::DMG;
use rgb_core::{cartridge::Cartridge, memory::Memory};
use std::fs;
use std::time::{Duration, Instant};

pub struct RomCartridge {
    data: Vec<u8>,
}

impl RomCartridge {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl Memory for RomCartridge {
    fn read_byte(&self, address: u16) -> u8 {
        let addr = address as usize;
        self.data.get(addr).copied().unwrap_or(0)
    }

    fn write_byte(&mut self, _address: u16, _value: u8) {}
}

struct TestCartridge(RomCartridge);

impl TestCartridge {
    fn new(data: Vec<u8>) -> Self {
        Self(RomCartridge::new(data))
    }
}

impl Memory for TestCartridge {
    fn read_byte(&self, address: u16) -> u8 {
        self.0.read_byte(address)
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        self.0.write_byte(address, value)
    }
}

impl Cartridge for TestCartridge {}

fn run_rom(path: &str) -> String {
    let data = fs::read(path).expect("read rom");
    let rom = Box::new(TestCartridge::new(data));
    let mut gb = DMG::new(rom);

    const MAX_FRAMES: usize = 10_000_000;
    const TIMEOUT: Duration = Duration::from_secs(60);

    let mut last_serial_len = 0;
    let start = Instant::now();

    for frame in 0..MAX_FRAMES {
        gb.step_frame();

        let serial = gb.serial();
        let len = serial.len();
        if len != last_serial_len {
            last_serial_len = len;
            let out = gb.serial_output();
            if out.contains("Passed") || out.contains("Failed") {
                return out;
            }
        }

        if start.elapsed() >= TIMEOUT {
            let out = gb.serial_output();
            panic!(
                "Blargg test timed out after {:?} (frames executed: {}). Latest output:\n{}",
                TIMEOUT,
                frame + 1,
                out
            );
        }
    }

    let out = gb.serial_output();
    panic!(
        "Blargg test exhausted {} frames without reaching a verdict. Latest output:\n{}",
        MAX_FRAMES, out
    );
}

pub fn run_blargg_rom(relative_path: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let full_path = format!("{manifest_dir}/tests/blargg-test-roms/{relative_path}");
    run_rom(&full_path)
}

pub fn assert_passed(output: &str) {
    assert!(
        output.contains("Passed") && !output.contains("Failed"),
        "Test output did not indicate success:\n{output}"
    );
}
