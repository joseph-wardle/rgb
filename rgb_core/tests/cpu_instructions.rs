mod common;

use common::{assert_passed, run_blargg_rom};

fn cpu_instrs_path(name: &str) -> String {
    format!("cpu_instrs/individual/{name}")
}

#[test]
fn cpu_instrs_special() {
    let out = run_blargg_rom(&cpu_instrs_path("01-special.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_interrupts() {
    let out = run_blargg_rom(&cpu_instrs_path("02-interrupts.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_op_sp_hl() {
    let out = run_blargg_rom(&cpu_instrs_path("03-op sp,hl.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_op_r_imm() {
    let out = run_blargg_rom(&cpu_instrs_path("04-op r,imm.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_op_rp() {
    let out = run_blargg_rom(&cpu_instrs_path("05-op rp.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_ld_r_r() {
    let out = run_blargg_rom(&cpu_instrs_path("06-ld r,r.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_jr_jp_call_ret_rst() {
    let out = run_blargg_rom(&cpu_instrs_path("07-jr,jp,call,ret,rst.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_misc_instrs() {
    let out = run_blargg_rom(&cpu_instrs_path("08-misc instrs.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_op_r_r() {
    let out = run_blargg_rom(&cpu_instrs_path("09-op r,r.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_bit_ops() {
    let out = run_blargg_rom(&cpu_instrs_path("10-bit ops.gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_op_a_hl() {
    let out = run_blargg_rom(&cpu_instrs_path("11-op a,(hl).gb"));
    assert_passed(&out);
}

#[test]
fn cpu_instrs_multi() {
    let out = run_blargg_rom("cpu_instrs/cpu_instrs.gb");
    assert_passed(&out);
}
