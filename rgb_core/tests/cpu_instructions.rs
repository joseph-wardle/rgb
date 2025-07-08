use std::fs;
use rgb_core::{cpu::CPU, mmu::MMU, serial::Serial, cartridge::Cartridge, memory::Memory};
use rgb_core::gameboy::DMG;

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

impl Cartridge for RomCartridge {}


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
    gb.run_until(|s| {
        let out = s.output_string();
        out.contains("Passed") || out.contains("Failed")
    }, 10_000_000);
    gb.bus.serial.output_string()
}

fn rom_path(name: &str) -> String {
    format!(
        "{}/tests/blargg-test-roms/cpu_instrs/individual/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    )
}

#[test]
fn test_01_special() {
    let out = run_rom(&rom_path("01-special.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_02_interrupts() {
    let out = run_rom(&rom_path("02-interrupts.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_03_op_sp_hl() {
    let out = run_rom(&rom_path("03-op sp,hl.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_04_op_r_imm() {
    let out = run_rom(&rom_path("04-op r,imm.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_05_op_rp() {
    let out = run_rom(&rom_path("05-op rp.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_06_ld_r_r() {
    let out = run_rom(&rom_path("06-ld r,r.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_07_jr_jp_call_ret_rst() {
    let out = run_rom(&rom_path("07-jr,jp,call,ret,rst.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_08_misc_instrs() {
    let out = run_rom(&rom_path("08-misc instrs.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_09_op_r_r() {
    let out = run_rom(&rom_path("09-op r,r.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_10_bit_ops() {
    let out = run_rom(&rom_path("10-bit ops.gb"));
    assert!(out.contains("Passed"), "{}", out);
}

#[test]
fn test_11_op_a_hl() {
    let out = run_rom(&rom_path("11-op a,(hl).gbv"));
    assert!(out.contains("Passed"), "{}", out);
}

