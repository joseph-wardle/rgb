mod common;

use common::{assert_passed, run_blargg_rom};

fn mem_timing_2_path(name: &str) -> String {
    format!("mem_timing-2/rom_singles/{name}")
}

#[test]
fn mem_timing_2_suite() {
    let out = run_blargg_rom("mem_timing-2/mem_timing.gb");
    assert_passed(&out);
}

#[test]
fn mem_timing_2_read() {
    let out = run_blargg_rom(&mem_timing_2_path("01-read_timing.gb"));
    assert_passed(&out);
}

#[test]
fn mem_timing_2_write() {
    let out = run_blargg_rom(&mem_timing_2_path("02-write_timing.gb"));
    assert_passed(&out);
}

#[test]
fn mem_timing_2_modify() {
    let out = run_blargg_rom(&mem_timing_2_path("03-modify_timing.gb"));
    assert_passed(&out);
}
