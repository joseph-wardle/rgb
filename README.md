# rgb

`rgb` is an educational Game Boy (DMG) emulator written in Rust.  The top
priority is clarity: every module should be approachable and serve as a
readable tour through the hardware rather than an exercise in clever
abstractions.

## Project Goals

- Keep the codebase small, focused, and documented so newcomers can follow
  along without cross-referencing multiple sources.
- Mirror the original hardware structure (CPU, MMU, PPU, APU, timers, input)
  with straightforward APIs and explicit data flow.
- Pair readable implementations with references to Pan Docs and community
  guides.

## What Works

| Component          | Notes                                                     |
|--------------------|-----------------------------------------------------------|
| SM83 CPU           | Full instruction set, HALT/STOP, IME, interrupt dispatch  |
| PPU                | BG, window, sprites, palettes, STAT/VBlank interrupts,   |
|                    | VRAM/OAM mode-based access restrictions                   |
| Timer              | DIV/TIMA/TMA/TAC with correct overflow and reload         |
| APU                | All four channels, frame sequencer, stereo output (cpal)  |
| OAM DMA            | Instantaneous copy model                                  |
| Joypad             | Polling model; joypad interrupt                           |
| MBC1/MBC3/MBC5     | ROM and RAM banking; battery-backed save files            |
| Boot ROM           | Optional; emulator starts at post-boot state by default   |

## Known Gaps

- **Mode 3 variable timing**: the pixel pipeline always takes 172 dots;
  the SCX fine-scroll, sprite, and window penalties are not yet modelled.
- **TIMA reload glitch**: on hardware, the reload from TMA and the timer
  interrupt are delayed by one machine cycle after TIMA overflow.
- **OAM DMA bus lock**: DMA is instantaneous here; the 160-µs CPU-bus
  lockout is not modelled.
- **STAT write quirk**: writing to STAT during certain modes can trigger a
  spurious interrupt on real hardware; not modelled.

## Building

```sh
cargo build --release
```

**Note for Linux users without ALSA development headers** (common on
RHEL/CentOS with only `alsa-lib` installed): a shim `pkgconfig/alsa.pc`
is included so `cargo build` works out of the box.  On Debian/Ubuntu you
can also install `libasound2-dev` instead.

## Running

```sh
cargo run --release -- <rom.gb>
cargo run --release -- <rom.gb> --boot-rom <dmg_boot.bin>
```

Controls: Arrow keys → D-pad, Z → B, X → A, Enter → Start,
Right Shift → Select, Escape → quit.

## Testing

The Blargg `cpu_instrs` test suite is used for CPU correctness.  ROMs are
downloaded automatically on first run (requires internet access) or can be
pre-placed in `target/blargg-test-roms/`:

```sh
cargo test --release -p rgb_core
```

## Contributing

Aim for changes that keep the learning experience front and centre.  New
code should explain the hardware, not just implement it.
