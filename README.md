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

If you’d like to contribute, aim for changes that keep the learning experience front and centre.