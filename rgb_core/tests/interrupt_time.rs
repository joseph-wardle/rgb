mod common;

use common::{assert_passed, run_blargg_rom};

#[test]
#[ignore = "CGB-only test (REQUIRE_CGB=1); requires CGB hardware mode"]
fn interrupt_time_suite() {
    let out = run_blargg_rom("interrupt_time/interrupt_time.gb");
    assert_passed(&out);
}
