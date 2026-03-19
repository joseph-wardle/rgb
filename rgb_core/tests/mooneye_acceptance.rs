//! Mooneye-test-suite acceptance tests (DMG-compatible subset).
//!
//! Each test runs a mooneye ROM and asserts the PASS Fibonacci register
//! signature (B=3 C=5 D=8 E=13 H=21 L=34).
//!
//! Tests that require hardware features not yet implemented are marked
//! `#[ignore]` with a brief reason.  Run them with:
//!
//! ```
//! cargo test -p rgb_core --test mooneye_acceptance -- --ignored
//! ```
//!
//! ## Suite sources
//!
//! Mooneye-test-suite by Joonas Javanainen (Gekkio) with contributions from
//! Wilbert Pol and others: <https://github.com/Gekkio/mooneye-test-suite>

mod common;

use common::run_mooneye_rom;

fn acc(path: &str) -> String {
    format!("acceptance/{path}")
}

// ============================================================================
// acceptance/bits
// ============================================================================

#[test]
fn bits_mem_oam() {
    assert!(run_mooneye_rom(&acc("bits/mem_oam.gb")));
}

#[test]
fn bits_reg_f() {
    assert!(run_mooneye_rom(&acc("bits/reg_f.gb")));
}

#[test]
fn bits_unused_hwio_gs() {
    assert!(run_mooneye_rom(&acc("bits/unused_hwio-GS.gb")));
}

// ============================================================================
// acceptance/instr
// ============================================================================

#[test]
fn instr_daa() {
    assert!(run_mooneye_rom(&acc("instr/daa.gb")));
}

// ============================================================================
// acceptance/interrupts
// ============================================================================

#[test]
fn interrupts_ie_push() {
    assert!(run_mooneye_rom(&acc("interrupts/ie_push.gb")));
}

// ============================================================================
// acceptance/oam_dma
// ============================================================================

#[test]
fn oam_dma_basic() {
    assert!(run_mooneye_rom(&acc("oam_dma/basic.gb")));
}

#[test]
fn oam_dma_reg_read() {
    assert!(run_mooneye_rom(&acc("oam_dma/reg_read.gb")));
}

#[test]
fn oam_dma_sources_gs() {
    assert!(run_mooneye_rom(&acc("oam_dma/sources-GS.gb")));
}

// ============================================================================
// acceptance/ppu — pending sub-scanline PPU timing
// ============================================================================

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_hblank_ly_scx_timing_gs() {
    assert!(run_mooneye_rom(&acc("ppu/hblank_ly_scx_timing-GS.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_intr_1_2_timing_gs() {
    assert!(run_mooneye_rom(&acc("ppu/intr_1_2_timing-GS.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_intr_2_0_timing() {
    assert!(run_mooneye_rom(&acc("ppu/intr_2_0_timing.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_intr_2_mode0_timing() {
    assert!(run_mooneye_rom(&acc("ppu/intr_2_mode0_timing.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_intr_2_mode0_timing_sprites() {
    assert!(run_mooneye_rom(&acc("ppu/intr_2_mode0_timing_sprites.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_intr_2_mode3_timing() {
    assert!(run_mooneye_rom(&acc("ppu/intr_2_mode3_timing.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_intr_2_oam_ok_timing() {
    assert!(run_mooneye_rom(&acc("ppu/intr_2_oam_ok_timing.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_lcdon_timing_gs() {
    assert!(run_mooneye_rom(&acc("ppu/lcdon_timing-GS.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_lcdon_write_timing_gs() {
    assert!(run_mooneye_rom(&acc("ppu/lcdon_write_timing-GS.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_stat_irq_blocking() {
    assert!(run_mooneye_rom(&acc("ppu/stat_irq_blocking.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_stat_lyc_onoff() {
    assert!(run_mooneye_rom(&acc("ppu/stat_lyc_onoff.gb")));
}

#[test]
#[ignore = "pending: sub-scanline PPU timing not implemented"]
fn ppu_vblank_stat_intr_gs() {
    assert!(run_mooneye_rom(&acc("ppu/vblank_stat_intr-GS.gb")));
}

// ============================================================================
// acceptance/serial
// ============================================================================

#[test]
#[ignore = "boot ROM not emulated; serial clock alignment test requires boot-ROM timing"]
fn serial_boot_sclk_align_dmgabcmgb() {
    assert!(run_mooneye_rom(&acc("serial/boot_sclk_align-dmgABCmgb.gb")));
}

// ============================================================================
// acceptance/timer
// ============================================================================

#[test]
fn timer_div_write() {
    assert!(run_mooneye_rom(&acc("timer/div_write.gb")));
}

#[test]
fn timer_rapid_toggle() {
    assert!(run_mooneye_rom(&acc("timer/rapid_toggle.gb")));
}

#[test]
fn timer_tim00() {
    assert!(run_mooneye_rom(&acc("timer/tim00.gb")));
}

#[test]
fn timer_tim00_div_trigger() {
    assert!(run_mooneye_rom(&acc("timer/tim00_div_trigger.gb")));
}

#[test]
fn timer_tim01() {
    assert!(run_mooneye_rom(&acc("timer/tim01.gb")));
}

#[test]
fn timer_tim01_div_trigger() {
    assert!(run_mooneye_rom(&acc("timer/tim01_div_trigger.gb")));
}

#[test]
fn timer_tim10() {
    assert!(run_mooneye_rom(&acc("timer/tim10.gb")));
}

#[test]
fn timer_tim10_div_trigger() {
    assert!(run_mooneye_rom(&acc("timer/tim10_div_trigger.gb")));
}

#[test]
fn timer_tim11() {
    assert!(run_mooneye_rom(&acc("timer/tim11.gb")));
}

#[test]
fn timer_tim11_div_trigger() {
    assert!(run_mooneye_rom(&acc("timer/tim11_div_trigger.gb")));
}

#[test]
fn timer_tima_reload() {
    assert!(run_mooneye_rom(&acc("timer/tima_reload.gb")));
}

#[test]
fn timer_tima_write_reloading() {
    assert!(run_mooneye_rom(&acc("timer/tima_write_reloading.gb")));
}

#[test]
fn timer_tma_write_reloading() {
    assert!(run_mooneye_rom(&acc("timer/tma_write_reloading.gb")));
}

// ============================================================================
// acceptance/ (top-level)
// ============================================================================

// --- Boot ROM tests — require the boot ROM to run first ----------------------

#[test]
#[ignore = "boot ROM not emulated; initial register and IO state differs"]
fn boot_div_dmgabcmgb() {
    assert!(run_mooneye_rom(&acc("boot_div-dmgABCmgb.gb")));
}

#[test]
#[ignore = "boot ROM not emulated; initial register and IO state differs"]
fn boot_hwio_dmgabcmgb() {
    assert!(run_mooneye_rom(&acc("boot_hwio-dmgABCmgb.gb")));
}

#[test]
#[ignore = "boot ROM not emulated; initial register and IO state differs"]
fn boot_regs_dmgabc() {
    assert!(run_mooneye_rom(&acc("boot_regs-dmgABC.gb")));
}

// --- Instruction timing ------------------------------------------------------

#[test]
fn add_sp_e_timing() {
    assert!(run_mooneye_rom(&acc("add_sp_e_timing.gb")));
}

#[test]
fn call_cc_timing() {
    assert!(run_mooneye_rom(&acc("call_cc_timing.gb")));
}

#[test]
fn call_cc_timing2() {
    assert!(run_mooneye_rom(&acc("call_cc_timing2.gb")));
}

#[test]
fn call_timing() {
    assert!(run_mooneye_rom(&acc("call_timing.gb")));
}

#[test]
fn call_timing2() {
    assert!(run_mooneye_rom(&acc("call_timing2.gb")));
}

#[test]
fn jp_cc_timing() {
    assert!(run_mooneye_rom(&acc("jp_cc_timing.gb")));
}

#[test]
fn jp_timing() {
    assert!(run_mooneye_rom(&acc("jp_timing.gb")));
}

#[test]
fn ld_hl_sp_e_timing() {
    assert!(run_mooneye_rom(&acc("ld_hl_sp_e_timing.gb")));
}

#[test]
fn pop_timing() {
    assert!(run_mooneye_rom(&acc("pop_timing.gb")));
}

#[test]
fn push_timing() {
    assert!(run_mooneye_rom(&acc("push_timing.gb")));
}

#[test]
fn ret_cc_timing() {
    assert!(run_mooneye_rom(&acc("ret_cc_timing.gb")));
}

#[test]
fn ret_timing() {
    assert!(run_mooneye_rom(&acc("ret_timing.gb")));
}

#[test]
fn rst_timing() {
    assert!(run_mooneye_rom(&acc("rst_timing.gb")));
}

// --- Interrupt / IME timing --------------------------------------------------

#[test]
fn di_timing_gs() {
    assert!(run_mooneye_rom(&acc("di_timing-GS.gb")));
}

#[test]
fn ei_sequence() {
    assert!(run_mooneye_rom(&acc("ei_sequence.gb")));
}

#[test]
fn ei_timing() {
    assert!(run_mooneye_rom(&acc("ei_timing.gb")));
}

#[test]
fn if_ie_registers() {
    assert!(run_mooneye_rom(&acc("if_ie_registers.gb")));
}

#[test]
fn intr_timing() {
    assert!(run_mooneye_rom(&acc("intr_timing.gb")));
}

#[test]
fn rapid_di_ei() {
    assert!(run_mooneye_rom(&acc("rapid_di_ei.gb")));
}

#[test]
fn reti_intr_timing() {
    assert!(run_mooneye_rom(&acc("reti_intr_timing.gb")));
}

#[test]
fn reti_timing() {
    assert!(run_mooneye_rom(&acc("reti_timing.gb")));
}

// --- HALT behaviour ----------------------------------------------------------

#[test]
fn halt_ime0_ei() {
    assert!(run_mooneye_rom(&acc("halt_ime0_ei.gb")));
}

#[test]
fn halt_ime0_nointr_timing() {
    assert!(run_mooneye_rom(&acc("halt_ime0_nointr_timing.gb")));
}

#[test]
fn halt_ime1_timing() {
    assert!(run_mooneye_rom(&acc("halt_ime1_timing.gb")));
}

#[test]
fn halt_ime1_timing2_gs() {
    assert!(run_mooneye_rom(&acc("halt_ime1_timing2-GS.gb")));
}

// --- OAM DMA -----------------------------------------------------------------

#[test]
fn oam_dma_restart() {
    assert!(run_mooneye_rom(&acc("oam_dma_restart.gb")));
}

#[test]
fn oam_dma_start() {
    assert!(run_mooneye_rom(&acc("oam_dma_start.gb")));
}

#[test]
fn oam_dma_timing() {
    assert!(run_mooneye_rom(&acc("oam_dma_timing.gb")));
}
