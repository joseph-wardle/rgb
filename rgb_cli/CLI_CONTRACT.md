# RGB CLI Contract (M1)

This document defines the command-line interface for Milestone 1 ("Runnable Host Flow").
Implementation code should follow this contract exactly unless this file is intentionally updated.

## Goals
- Keep invocation obvious and easy to remember.
- Keep options small, explicit, and beginner-friendly.
- Keep runtime behavior deterministic and easy to debug.

## Canonical Invocation

During development:

```bash
cargo run -p rgb_cli -- [OPTIONS] <ROM_PATH>
```

Binary form:

```bash
rgb_cli [OPTIONS] <ROM_PATH>
```

## Arguments and Options

### Positional
- `<ROM_PATH>` (required)
  - Path to a Game Boy ROM file.
  - The command fails if the file does not exist, is unreadable, or is not a supported cartridge.

### Options
- `--frames <N>`
  - Optional frame limit.
  - `N` must be a positive integer (`N >= 1`).
  - If omitted, the emulator runs until externally interrupted.

- `--boot <MODE>`
  - Optional boot mode selector.
  - Allowed values:
    - `cold`
    - `post-bios`
  - Default: `post-bios`
  - Mapping:
    - `cold` -> `DMG::new(...)`
    - `post-bios` -> `DMG::new_post_bios(...)`

- `--serial <MODE>`
  - Optional serial output mode.
  - Allowed values:
    - `off`: do not print serial output.
    - `live`: print serial bytes as they are produced.
    - `final`: print full serial buffer once on exit.
  - Default: `off`

- `--quiet`
  - Suppress normal startup/status logging.
  - Does not suppress errors.
  - Does not suppress serial output requested via `--serial`.

- `--trace`
  - Enable runtime trace logging.
  - This is only valid when the binary is built with the `trace` feature in `rgb_core`.
  - If not available in the current build, fail with a clear message describing how to enable it.

- `-h`, `--help`
  - Print usage and exit successfully.

- `-V`, `--version`
  - Print version and exit successfully.

## Parsing and Validation Rules
- Unknown flags are rejected.
- Missing values for value-taking options are rejected.
- Invalid enums (`--boot`, `--serial`) are rejected.
- Invalid numeric values for `--frames` are rejected.
- Duplicate single-value options are rejected (`--frames`, `--boot`, `--serial`).
- `--` ends option parsing; remaining token is treated as positional input.

## Exit Codes
- `0`: success.
- `1`: runtime/setup failure (IO, ROM parse, unsupported mapper at runtime, etc.).
- `2`: CLI usage/validation failure.

## Output Contract
- Normal mode prints a concise startup block:
  - `Run Start`
  - ROM path + cartridge title/mapper
  - boot mode
  - limits summary (frames, serial mode, trace mode)
- Long headless runs may print periodic progress lines (`progress: frame ...`) to show forward motion.
- Normal mode prints a concise shutdown block on explicit stop:
  - `Run Complete`
  - frames executed
  - stop reason
  - serial byte count
- `--quiet` suppresses startup/progress/shutdown status output.
- Error messages remain single-purpose and actionable.

## Non-Goals for M1
- No interactive key mapping UI yet.
- No graphics window integration requirement in this contract.
- No audio output requirement in this contract.
- No debugger command shell in this contract.
