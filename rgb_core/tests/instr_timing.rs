mod common;

use common::{assert_passed, run_blargg_rom};

#[test]
fn instr_timing_suite() {
    let out = run_blargg_rom("instr_timing/instr_timing.gb");
    assert_passed(&out);
}
