# RGB

`rgb` is an educational Game Boy emulator written in Rust. The project’s top priority is clarity: every module should be approachable, easy to read, and serve as a tour through the hardware rather than an exercise in clever abstractions. This is a hobby project, so performance optimizations and feature completeness are not primary concerns.

## Project Goals
- Keep the codebase small, focused, and heavily documented so newcomers can follow along without cross-referencing multiple sources.
- Mirror the original hardware structure (CPU, MMU, PPU, APU, timers, input) with straightforward APIs and explicit data flow.
- Provide a friendly study resource by pairing readable implementations with references back to Pan Docs and other community guides.

## Roadmap (High Level)
- [x] Bootstrap the workspace layout (`rgb_core`, `rgb_cli`) and CI-friendly tooling.
- [x] Implement the baseline CPU fetch/decode/execute loop with opcode tables and helpers.
- [ ] Tighten CPU timing, interrupts, and HALT/STOP quirks;
- [ ] Flesh out the MMU (timers, DMA, IO) with readable docs and per-register tests.
- [ ] Build an accurate, well-commented PPU pipeline and framebuffer interface.
- [ ] Implement the four APU channels, frame sequencer, and a simple host audio backend.
- [ ] Replace the CLI placeholder with a ROM runner, trace/debug toggles, and input handling.
- [ ] Grow documentation into a guided tour of the hardware and code structure.

## MVP Milestone Board (First Pass)
The board below is the practical path to a full first-pass emulator that is functional top-to-bottom while staying small, readable, and educational.

### M1. Runnable Host Flow (`rgb_cli`)
- [ ] Replace the placeholder CLI with ROM loading, startup config, and a clear main loop.
- [ ] Add debug/trace toggles that help learning without cluttering default output.
- [ ] Ensure errors are user-friendly (bad path, unsupported mapper, invalid ROM, etc.).

### M2. Stable Core API (`rgb_core`)
- [ ] Define a compact emulator-facing API for stepping and host integration.
- [ ] Expose clean access points for framebuffer, input updates, and status.
- [ ] Keep internals private unless they are part of the teaching surface.

### M3. MMU + IO Baseline Correctness
- [ ] Finalize register defaults and read/write semantics for core IO ranges.
- [ ] Make unmapped/unused behavior explicit and documented.
- [ ] Add per-register tests for timer, interrupt, and key LCD/joypad paths.

### M4. Interrupts + CPU Timing Polish
- [ ] Tighten IF/IE behavior and interrupt priority/servicing details.
- [ ] Finish HALT/STOP edge cases and IME scheduling correctness.
- [ ] Unignore timing-related CPU tests as behavior lands.

### M5. Timer Hardware Accuracy
- [ ] Implement accurate `DIV/TIMA/TMA/TAC` behavior, including tricky edge cases.
- [ ] Verify overflow/reload timing against reference tests.
- [ ] Keep timer code heavily commented with hardware rationale.

### M6. DMA + Memory Timing Integration
- [ ] Implement OAM DMA transfer flow from `FF46`.
- [ ] Wire DMA effects into memory accessibility/timing behavior.
- [ ] Enable relevant timing/OAM test suites once stable.

### M7. PPU Timing State Machine
- [ ] Implement LY progression, PPU modes, and frame/scanline timing.
- [ ] Implement VBlank + STAT interrupt behavior (`LYC`, mode interrupts, etc.).
- [ ] Document the control flow as a hardware walkthrough.

### M8. First-Pass Pixel Pipeline + Framebuffer
- [ ] Render background/window/sprites with correct priority rules for MVP.
- [ ] Produce a stable framebuffer interface for the host frontend.
- [ ] Validate with basic visual test ROMs and in-game sanity checks.

### M9. Input Path End-to-End
- [ ] Connect host key input to joypad matrix/select behavior.
- [ ] Request joypad interrupts correctly on transitions.
- [ ] Validate with gameplay-level checks (menus, movement, start/select usage).

### M10. Cartridge Compatibility for Real Games
- [ ] Keep ROM-only + MBC1 robust.
- [ ] Add next high-impact mappers (at minimum MBC3 and MBC5) for practical compatibility.
- [ ] Add battery-backed RAM save/load behavior.

### M11. APU MVP
- [ ] Implement enough APU register behavior for software expectations.
- [ ] Add a simple audio output path (or clearly documented temporary mute mode for MVP if needed).
- [ ] Unignore the first sound tests that become valid.

### M12. CI, Docs, and MVP Release Cut
- [ ] Ensure CI runs the intended workspace tests and catches regressions.
- [ ] Promote ignored suites milestone-by-milestone as features are completed.
- [ ] Do a final documentation pass and tag the first playable MVP release with known limitations.

If you’d like to contribute, aim for changes that keep the learning experience front and centre.
