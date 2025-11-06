use rgb_core::gameboy::DMG;
use rgb_core::{cartridge::Cartridge, memory::Memory};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const BLARGG_ARCHIVE_URL: &str =
    "https://github.com/retrio/gb-test-roms/archive/refs/heads/master.zip";
const BLARGG_ARCHIVE_ROOT: &str = "gb-test-roms-master";
const READY_MARKER: &str = ".rgb_blargg_ready";

static BLARGG_ROM_DIR: OnceLock<PathBuf> = OnceLock::new();

pub struct RomCartridge {
    data: Vec<u8>,
}

impl RomCartridge {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl Memory for RomCartridge {
    fn read_byte(&self, address: u16) -> u8 {
        let addr = address as usize;
        self.data.get(addr).copied().unwrap_or(0)
    }

    fn write_byte(&mut self, _address: u16, _value: u8) {}
}

struct TestCartridge(RomCartridge);

impl TestCartridge {
    fn new(data: Vec<u8>) -> Self {
        Self(RomCartridge::new(data))
    }
}

impl Memory for TestCartridge {
    fn read_byte(&self, address: u16) -> u8 {
        self.0.read_byte(address)
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        self.0.write_byte(address, value)
    }
}

impl Cartridge for TestCartridge {}

fn run_rom(path: &Path) -> String {
    let data = fs::read(path).unwrap_or_else(|err| {
        panic!("failed to read ROM from {}: {err}", path.display());
    });
    let rom = Box::new(TestCartridge::new(data));
    let mut gb = DMG::new(rom);

    const MAX_FRAMES: usize = 10_000_000;
    const TIMEOUT: Duration = Duration::from_secs(240);

    let mut last_serial_len = 0;
    let start = Instant::now();

    for frame in 0..MAX_FRAMES {
        gb.step_frame();

        let serial = gb.serial();
        let len = serial.len();
        if len != last_serial_len {
            last_serial_len = len;
            let out = gb.serial_output();
            if out.contains("Passed") || out.contains("Failed") {
                return out;
            }
        }

        if start.elapsed() >= TIMEOUT {
            let out = gb.serial_output();
            panic!(
                "Blargg test timed out after {:?} (frames executed: {}). Latest output:\n{}",
                TIMEOUT,
                frame + 1,
                out
            );
        }
    }

    let out = gb.serial_output();
    panic!(
        "Blargg test exhausted {} frames without reaching a verdict. Latest output:\n{}",
        MAX_FRAMES, out
    );
}

pub fn run_blargg_rom(relative_path: &str) -> String {
    let path = full_path(relative_path);
    run_rom(&path)
}

pub fn assert_passed(output: &str) {
    assert!(
        output.contains("Passed") && !output.contains("Failed"),
        "Test output did not indicate success:\n{output}"
    );
}

fn full_path(relative_path: &str) -> PathBuf {
    let base = blargg_rom_dir();
    let full = base.join(relative_path);
    if full.exists() {
        full
    } else {
        panic!(
            "blargg ROM {} not found under {}",
            relative_path,
            base.display()
        );
    }
}

fn blargg_rom_dir() -> &'static PathBuf {
    BLARGG_ROM_DIR.get_or_init(|| {
        let dir = rom_root_dir();
        if !blargg_roms_ready(&dir) {
            if let Err(err) = download_blargg_roms(&dir) {
                panic!(
                    "failed to download blargg test ROMs into {}: {err}",
                    dir.display()
                );
            }
        }
        dir
    })
}

fn rom_root_dir() -> PathBuf {
    if let Ok(path) = env::var("RGB_BLARGG_ROM_DIR") {
        return PathBuf::from(path);
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            manifest_dir
                .parent()
                .map(|parent| parent.join("target"))
                .unwrap_or_else(|| manifest_dir.join("target"))
        });

    target_dir.join("blargg-test-roms")
}

fn blargg_roms_ready(dir: &Path) -> bool {
    if dir.join(READY_MARKER).exists() {
        return true;
    }

    dir.join("cpu_instrs/cpu_instrs.gb").exists()
}

fn download_blargg_roms(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| format!("create {}: {err}", dir.display()))?;

    let response = ureq::get(BLARGG_ARCHIVE_URL)
        .call()
        .map_err(|err| format!("download blargg archive: {err}"))?;

    if !(200..300).contains(&response.status()) {
        return Err(format!(
            "unexpected HTTP status {} from {}",
            response.status(),
            BLARGG_ARCHIVE_URL
        ));
    }

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|err| format!("read response body: {err}"))?;

    let mut cursor = io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(&mut cursor).map_err(|err| format!("open zip archive: {err}"))?;

    let root = Path::new(BLARGG_ARCHIVE_ROOT);
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|err| format!("read zip entry {i}: {err}"))?;

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
                .map_err(|err| format!("create dir {}: {err}", out_path.display()))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create dir {}: {err}", parent.display()))?;
        }

        let mut outfile = fs::File::create(&out_path)
            .map_err(|err| format!("create file {}: {err}", out_path.display()))?;
        io::copy(&mut file, &mut outfile)
            .map_err(|err| format!("write file {}: {err}", out_path.display()))?;
    }

    fs::write(dir.join(READY_MARKER), b"").map_err(|err| format!("write ready marker: {err}"))?;

    Ok(())
}
