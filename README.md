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
- [x] Publish a CLI contract before implementation details: [`rgb_cli/CLI_CONTRACT.md`](rgb_cli/CLI_CONTRACT.md).
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

## CLI Usage (`rgb_cli`)
Current host runner usage for the first-pass MVP flow:

```bash
cargo run -p rgb_cli -- [OPTIONS] <ROM_PATH>
```

Examples:

```bash
# Basic run (post-bios boot, unbounded)
cargo run -p rgb_cli -- ./roms/tetris.gb

# Fixed-length headless run for repeatable checks
cargo run -p rgb_cli -- --quiet --frames 600 ./roms/tetris.gb

# Cold boot + live serial passthrough (useful for test ROMs)
cargo run -p rgb_cli -- --boot cold --serial live ./roms/cpu_instrs.gb

# Trace-enabled run (requires trace feature at build time)
cargo run -p rgb_cli --features trace -- --trace ./roms/tetris.gb
```

Supported flags:
- `--frames <N>`: stop after `N` frames (`N >= 1`), otherwise run until interrupted.
- `--boot <MODE>`: `cold` or `post-bios` (default: `post-bios`).
- `--serial <MODE>`: `off`, `live`, or `final` (default: `off`).
- `--quiet`: suppress startup/status logs (errors still print).
- `--trace`: enable trace logging; returns an actionable error if the binary was not built with `--features trace`.
- `-h`, `--help`: print usage text.
- `-V`, `--version`: print version.

Reference contract: [`rgb_cli/CLI_CONTRACT.md`](rgb_cli/CLI_CONTRACT.md)

## `rgb_cli` Architecture (Responsibility Boundaries)
- `main.rs`: process boundary only (stderr + exit code mapping).
- `lib.rs`: testable public entrypoints (`run`, `run_with_args`) and re-exports.
- `config.rs`: argument parsing and typed configuration model.
- `error.rs`: user-facing error types, categories, and exit code mapping.
- `rom.rs`: ROM file loading + cartridge metadata extraction.
- `emulator.rs`: explicit boot mode to `DMG` constructor mapping.
- `runner.rs`: deterministic frame loop, stop conditions, and serial/progress output behavior.
- `trace.rs`: optional trace setup and feature-gated behavior.
- `app.rs`: orchestration layer connecting parse -> load -> construct -> run.

If you’d like to contribute, aim for changes that keep the learning experience front and centre.
