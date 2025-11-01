mod common;

use common::{assert_passed, run_blargg_rom};

fn oam_path(name: &str) -> String {
    format!("oam_bug/rom_singles/{name}")
}

#[test]
fn oam_bug_suite() {
    let out = run_blargg_rom("oam_bug/oam_bug.gb");
    assert_passed(&out);
}

#[test]
fn oam_bug_lcd_sync() {
    let out = run_blargg_rom(&oam_path("1-lcd_sync.gb"));
    assert_passed(&out);
}

#[test]
fn oam_bug_causes() {
    let out = run_blargg_rom(&oam_path("2-causes.gb"));
    assert_passed(&out);
}

#[test]
fn oam_bug_non_causes() {
    let out = run_blargg_rom(&oam_path("3-non_causes.gb"));
    assert_passed(&out);
}

#[test]
fn oam_bug_scanline_timing() {
    let out = run_blargg_rom(&oam_path("4-scanline_timing.gb"));
    assert_passed(&out);
}

#[test]
fn oam_bug_timing_bug() {
    let out = run_blargg_rom(&oam_path("5-timing_bug.gb"));
    assert_passed(&out);
}

#[test]
fn oam_bug_timing_no_bug() {
    let out = run_blargg_rom(&oam_path("6-timing_no_bug.gb"));
    assert_passed(&out);
}

#[test]
fn oam_bug_timing_effect() {
    let out = run_blargg_rom(&oam_path("7-timing_effect.gb"));
    assert_passed(&out);
}

#[test]
fn oam_bug_instr_effect() {
    let out = run_blargg_rom(&oam_path("8-instr_effect.gb"));
    assert_passed(&out);
}
