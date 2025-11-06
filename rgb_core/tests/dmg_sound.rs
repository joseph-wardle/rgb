mod common;

use common::{assert_passed, run_blargg_rom};

fn dmg_sound_path(name: &str) -> String {
    format!("dmg_sound/rom_singles/{name}")
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_suite() {
    let out = run_blargg_rom("dmg_sound/dmg_sound.gb");
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_registers() {
    let out = run_blargg_rom(&dmg_sound_path("01-registers.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_len_ctr() {
    let out = run_blargg_rom(&dmg_sound_path("02-len ctr.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_trigger() {
    let out = run_blargg_rom(&dmg_sound_path("03-trigger.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_sweep() {
    let out = run_blargg_rom(&dmg_sound_path("04-sweep.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_sweep_details() {
    let out = run_blargg_rom(&dmg_sound_path("05-sweep details.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_overflow_on_trigger() {
    let out = run_blargg_rom(&dmg_sound_path("06-overflow on trigger.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_len_sweep_period_sync() {
    let out = run_blargg_rom(&dmg_sound_path("07-len sweep period sync.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_len_ctr_during_power() {
    let out = run_blargg_rom(&dmg_sound_path("08-len ctr during power.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_wave_read_while_on() {
    let out = run_blargg_rom(&dmg_sound_path("09-wave read while on.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_wave_trigger_while_on() {
    let out = run_blargg_rom(&dmg_sound_path("10-wave trigger while on.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_regs_after_power() {
    let out = run_blargg_rom(&dmg_sound_path("11-regs after power.gb"));
    assert_passed(&out);
}

#[test]
#[ignore = "requires DMG audio hardware emulation"]
fn dmg_sound_wave_write_while_on() {
    let out = run_blargg_rom(&dmg_sound_path("12-wave write while on.gb"));
    assert_passed(&out);
}
