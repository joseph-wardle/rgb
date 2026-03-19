//! Shared test infrastructure for rgb_core integration tests.
// Each test binary compiles its own copy of this module and uses only a subset
// of the functions defined here.  Suppress the resulting dead-code warnings.
#![allow(dead_code)]
//!
//! Two test suites are supported:
//!
//! | Suite   | Author(s)                             | Output protocol           |
//! |---------|---------------------------------------|---------------------------|
//! | Blargg  | Shay Green; additional by Wilbert Pol | Serial text / RAM at A000 |
//! | Mooneye | Joonas Javanainen + Wilbert Pol et al | LD B,B Fibonacci pattern  |
//!
//! ## Blargg protocol
//!
//! The ROM writes pass/fail text to the serial port, or writes a 3-byte magic
//! signature (0xDE 0xB0 0x61) at 0xA001–0xA003 plus a status byte at 0xA000
//! (0x80 = running), and null-terminated result text starting at 0xA004.
//!
//! ## Mooneye protocol
//!
//! The ROM signals completion by loading B=3, C=5, D=8, E=13, H=21, L=34
//! (Fibonacci) for PASS — or any other combination for FAIL — and executing
//! `LD B,B` (opcode 0x40) in a tight loop.  We detect completion once the
//! CPU's program counter has been stable across several consecutive frames.
//!
//! ## ROM caching
//!
//! ROMs are downloaded on first run and cached under `target/`.  Override
//! with `RGB_BLARGG_ROM_DIR` or `RGB_MOONEYE_ROM_DIR` (used by CI).

use rgb_core::cartridge::{Cartridge, CartridgeKind};
use rgb_core::gameboy::DMG;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

// ============================================================================
// Suite descriptors
// ============================================================================

struct Suite {
    name: &'static str,
    /// Environment variable that overrides the local cache directory.
    env_var: &'static str,
    /// Subdirectory name created under `target/` when no env var is set.
    default_dir: &'static str,
    /// URL of the ZIP archive to download on cache miss.
    archive_url: &'static str,
    /// Top-level directory inside the ZIP that is stripped on extraction.
    archive_root: &'static str,
    /// Marker file written after a successful extraction; presence = ready.
    ready_marker: &'static str,
}

/// Blargg's test ROMs, with additional tests by Wilbert Pol (halt_bug).
///
/// Source: <https://github.com/retrio/gb-test-roms>
const BLARGG: Suite = Suite {
    name: "blargg",
    env_var: "RGB_BLARGG_ROM_DIR",
    default_dir: "blargg-test-roms",
    archive_url: "https://github.com/retrio/gb-test-roms/archive/refs/heads/master.zip",
    archive_root: "gb-test-roms-master",
    ready_marker: ".rgb_blargg_ready",
};

/// Mooneye test suite by Joonas Javanainen (Gekkio) with contributions from
/// Wilbert Pol and others.  Uses the Fibonacci register protocol (see module
/// docs).
///
/// Pre-built ROMs are hosted at <https://gekkio.fi/files/mooneye-test-suite/>.
/// Source: <https://github.com/Gekkio/mooneye-test-suite>
///
/// Update `archive_url` and `archive_root` together if upgrading to a newer build.
const MOONEYE: Suite = Suite {
    name: "mooneye",
    env_var: "RGB_MOONEYE_ROM_DIR",
    default_dir: "mooneye-test-roms",
    archive_url: "https://gekkio.fi/files/mooneye-test-suite/mts-20240926-1737-443f6e1/mts-20240926-1737-443f6e1.zip",
    archive_root: "mts-20240926-1737-443f6e1",
    ready_marker: ".rgb_mooneye_ready",
};

static BLARGG_DIR: OnceLock<PathBuf> = OnceLock::new();
static MOONEYE_DIR: OnceLock<PathBuf> = OnceLock::new();

// ============================================================================
// Public API — Blargg
// ============================================================================

/// Run a blargg ROM and return its output text.
///
/// `relative_path` is resolved relative to the blargg ROM cache directory
/// (e.g. `"cpu_instrs/individual/01-special.gb"`).
pub fn run_blargg_rom(relative_path: &str) -> String {
    let path = suite_path(&BLARGG, &BLARGG_DIR, relative_path);
    run_blargg(&path)
}

/// Assert that a blargg output string indicates success.
pub fn assert_passed(output: &str) {
    assert!(
        output.contains("Passed") && !output.contains("Failed"),
        "Test output did not indicate success:\n{output}"
    );
}

// ============================================================================
// Public API — Mooneye
// ============================================================================

/// Run a mooneye ROM and return `true` if it signals PASS.
///
/// `relative_path` is resolved relative to the mooneye ROM cache directory
/// (e.g. `"acceptance/timer/div_write.gb"`).
///
/// Detection: once the CPU's PC has been stable across several consecutive
/// frames (the ROM is looping on `LD B,B`), we check the Fibonacci register
/// signature.  A timeout panic fires if the ROM never reaches a stable PC.
pub fn run_mooneye_rom(relative_path: &str) -> bool {
    let path = suite_path(&MOONEYE, &MOONEYE_DIR, relative_path);
    run_mooneye(&path)
}

// ============================================================================
// ROM executor — Blargg
// ============================================================================

fn run_blargg(path: &Path) -> String {
    let mut gb = load_rom(path);

    const MAX_FRAMES: usize = 10_000_000;
    const TIMEOUT: Duration = Duration::from_secs(60);
    let start = Instant::now();
    let mut last_serial_len = 0;

    for frame in 0..MAX_FRAMES {
        gb.step_frame();

        // Serial-output path: cpu_instrs, instr_timing, mem_timing, …
        let serial_len = gb.serial().len();
        if serial_len != last_serial_len {
            last_serial_len = serial_len;
            let out = gb.serial_output();
            if out.contains("Passed") || out.contains("Failed") {
                return out;
            }
        }

        // RAM-output path: halt_bug, mem_timing-2, oam_bug, dmg_sound, …
        // Blargg's shell writes a 3-byte signature at 0xA001–A003, a status
        // byte at 0xA000 (0x80 = still running), and text from 0xA004.
        if gb.peek_byte(0xA001) == 0xDE
            && gb.peek_byte(0xA002) == 0xB0
            && gb.peek_byte(0xA003) == 0x61
            && gb.peek_byte(0xA000) != 0x80
        {
            let mut out = String::new();
            for i in 0..256u16 {
                let b = gb.peek_byte(0xA004 + i);
                if b == 0 {
                    break;
                }
                out.push(b as char);
            }
            return out;
        }

        if start.elapsed() >= TIMEOUT {
            panic!(
                "blargg test timed out after {:?} (frames: {}). Serial output:\n{}",
                TIMEOUT,
                frame + 1,
                gb.serial_output()
            );
        }
    }

    panic!(
        "blargg test ran {} frames without a verdict. Serial output:\n{}",
        MAX_FRAMES,
        gb.serial_output()
    );
}

// ============================================================================
// ROM executor — Mooneye
// ============================================================================

fn run_mooneye(path: &Path) -> bool {
    let mut gb = load_rom(path);

    // A mooneye ROM loops on `LD B,B` (opcode 0x40) once it finishes.
    // We declare "done" once the CPU's PC has been identical for this many
    // consecutive frames — at 59.73 fps that's ~84 ms, negligible overhead.
    const STABLE_FRAMES_REQUIRED: u32 = 5;
    const MAX_FRAMES: usize = 10_000_000;
    const TIMEOUT: Duration = Duration::from_secs(60);

    let start = Instant::now();
    let mut prev_pc = u16::MAX;
    let mut stable_count = 0u32;

    for frame in 0..MAX_FRAMES {
        gb.step_frame();

        let pc = gb.cpu_pc();
        if pc == prev_pc {
            stable_count += 1;
        } else {
            stable_count = 0;
            prev_pc = pc;
        }

        // Mooneye ROMs signal completion by looping on `LD B,B; JR -2`
        // (bytes 0x40 0x18 0xFE).  Frame timing can land the PC on either
        // instruction, so we recognise the 3-byte pattern at prev_pc or at
        // prev_pc - 1.  This avoids false positives from LY-polling loops
        // that happen to be frame-synchronised.
        let done_pc = if gb.peek_byte(prev_pc) == 0x40
            && gb.peek_byte(prev_pc + 1) == 0x18
            && gb.peek_byte(prev_pc + 2) == 0xFE
        {
            // PC is on the `LD B,B` instruction.
            stable_count >= STABLE_FRAMES_REQUIRED
        } else if gb.peek_byte(prev_pc.wrapping_sub(1)) == 0x40
            && gb.peek_byte(prev_pc) == 0x18
            && gb.peek_byte(prev_pc + 1) == 0xFE
        {
            // PC is on the `JR -2` instruction.
            stable_count >= STABLE_FRAMES_REQUIRED
        } else {
            false
        };
        if done_pc {
            let pass = gb.mooneye_pass();
            if !pass {
                // Read HRAM diagnostic bytes left by the mooneye test framework
                // when a sub-test fails (test_addr, test_got, test_reg, test_mask).
                let addr = gb.peek_byte(0xFF80) as u16 | (gb.peek_byte(0xFF81) as u16) << 8;
                let got = gb.peek_byte(0xFF82);
                let reg = gb.peek_byte(0xFF83);
                let mask = gb.peek_byte(0xFF84);
                eprintln!(
                    "MOONEYE FAIL {}: test_addr=0x{addr:04X} reg=0x{reg:02X} \
                     mask=0x{mask:02X} got=0x{got:02X} regs=[{}]",
                    path.display(),
                    gb.mooneye_regs_debug()
                );
            }
            return pass;
        }

        if start.elapsed() >= TIMEOUT {
            panic!(
                "mooneye test timed out after {:?} (frames: {}): {}",
                TIMEOUT,
                frame + 1,
                path.display()
            );
        }
    }

    panic!(
        "mooneye test ran {} frames without a stable PC: {}",
        MAX_FRAMES,
        path.display()
    );
}

// ============================================================================
// Shared helpers
// ============================================================================

fn load_rom(path: &Path) -> DMG {
    let data = fs::read(path).unwrap_or_else(|err| {
        panic!("failed to read ROM {}: {err}", path.display());
    });
    let cartridge = CartridgeKind::from_bytes(data).unwrap_or_else(|err| {
        panic!("failed to parse cartridge {}: {err}", path.display());
    });
    DMG::new(Box::new(cartridge) as Box<dyn Cartridge>)
}

/// Resolve a suite-relative path, ensuring the suite is downloaded first.
fn suite_path(suite: &Suite, lock: &'static OnceLock<PathBuf>, relative: &str) -> PathBuf {
    let base = suite_dir(suite, lock);
    let full = base.join(relative);
    if full.exists() {
        return full;
    }
    panic!(
        "{} ROM '{}' not found under {}",
        suite.name,
        relative,
        base.display()
    );
}

/// Return (and initialise) the cache directory for a suite.
fn suite_dir(suite: &Suite, lock: &'static OnceLock<PathBuf>) -> &'static PathBuf {
    lock.get_or_init(|| {
        let dir = resolve_dir(suite);
        if !dir.join(suite.ready_marker).exists() {
            if let Err(err) = download_suite(suite, &dir) {
                panic!(
                    "failed to download {} test ROMs into {}: {err}",
                    suite.name,
                    dir.display()
                );
            }
        }
        dir
    })
}

/// Determine the on-disk cache directory for a suite.
fn resolve_dir(suite: &Suite) -> PathBuf {
    if let Ok(path) = env::var(suite.env_var) {
        return PathBuf::from(path);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            manifest
                .parent()
                .map(|p| p.join("target"))
                .unwrap_or_else(|| manifest.join("target"))
        });
    target.join(suite.default_dir)
}

/// Download and extract a suite's ZIP archive into `dir`.
fn download_suite(suite: &Suite, dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    let response = ureq::get(suite.archive_url)
        .call()
        .map_err(|e| format!("download {} archive: {e}", suite.name))?;

    if !(200..300).contains(&response.status()) {
        return Err(format!(
            "unexpected HTTP {} from {}",
            response.status(),
            suite.archive_url
        ));
    }

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read response body: {e}"))?;

    let mut cursor = io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(&mut cursor).map_err(|e| format!("open zip: {e}"))?;

    let root = Path::new(suite.archive_root);
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {i}: {e}"))?;

        let Some(name) = file.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let Ok(relative) = name.strip_prefix(root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }

        let out_path = dir.join(relative);
        if file.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("mkdir {}: {e}", out_path.display()))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        let mut outfile = fs::File::create(&out_path)
            .map_err(|e| format!("create {}: {e}", out_path.display()))?;
        io::copy(&mut file, &mut outfile)
            .map_err(|e| format!("write {}: {e}", out_path.display()))?;
    }

    fs::write(dir.join(suite.ready_marker), b"").map_err(|e| format!("write ready marker: {e}"))?;

    Ok(())
}
