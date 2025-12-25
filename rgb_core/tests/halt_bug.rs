mod common;

use common::{assert_passed, run_blargg_rom};

#[test]
#[ignore = "requires better timing emulation"]
fn halt_bug_rom() {
    let out = run_blargg_rom("halt_bug.gb");
    assert_passed(&out);
}
