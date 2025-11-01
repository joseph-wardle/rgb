mod common;

use common::{assert_passed, run_blargg_rom};

fn mem_timing_path(name: &str) -> String {
    format!("mem_timing/individual/{name}")
}

#[test]
fn mem_timing_suite() {
    let out = run_blargg_rom("mem_timing/mem_timing.gb");
    assert_passed(&out);
}

#[test]
fn mem_timing_read() {
    let out = run_blargg_rom(&mem_timing_path("01-read_timing.gb"));
    assert_passed(&out);
}

#[test]
fn mem_timing_write() {
    let out = run_blargg_rom(&mem_timing_path("02-write_timing.gb"));
    assert_passed(&out);
}

#[test]
fn mem_timing_modify() {
    let out = run_blargg_rom(&mem_timing_path("03-modify_timing.gb"));
    assert_passed(&out);
}
