use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering::Relaxed};

pub static COLOR: AtomicBool = AtomicBool::new(true);
pub static CACHE: AtomicBool = AtomicBool::new(true);
pub static BINARY: AtomicBool = AtomicBool::new(true);
pub static DECIMALS: AtomicU8 = AtomicU8::new(2);
pub static PCT_COLOR: AtomicBool = AtomicBool::new(true);
pub static PCT_WARN: AtomicU8 = AtomicU8::new(50);
pub static PCT_CRIT: AtomicU8 = AtomicU8::new(80);

pub fn color_on() -> bool {
    COLOR.load(Relaxed)
}

pub fn sgr(params: &str) -> String {
    if color_on() && !params.is_empty() {
        format!("\x1b[{}m", params)
    } else {
        String::new()
    }
}

pub fn reset() -> String {
    if color_on() { "\x1b[m".to_string() } else { String::new() }
}

fn base_color(s: &str) -> Option<u8> {
    Some(match s {
        "black" => 0,
        "red" => 1,
        "green" => 2,
        "yellow" => 3,
        "blue" => 4,
        "magenta" => 5,
        "cyan" => 6,
        "white" => 7,
        _ => return None,
    })
}

/// `#rrggbb` or a base color name to RGB.
pub fn rgb_of(spec: &str) -> Option<(u8, u8, u8)> {
    let s = spec.trim().to_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            return Some((
                u8::from_str_radix(&hex[0..2], 16).ok()?,
                u8::from_str_radix(&hex[2..4], 16).ok()?,
                u8::from_str_radix(&hex[4..6], 16).ok()?,
            ));
        }
        return None;
    }
    Some(match s.as_str() {
        "black" => (0, 0, 0),
        "red" => (205, 49, 49),
        "green" => (13, 188, 121),
        "yellow" => (229, 229, 16),
        "blue" => (36, 114, 200),
        "magenta" => (188, 63, 188),
        "cyan" => (17, 168, 205),
        "white" => (229, 229, 229),
        _ => return None,
    })
}

fn single_color(tok: &str, bg: bool) -> Option<String> {
    let (fg_base, bright_base, ext) = if bg { (40, 100, 48) } else { (30, 90, 38) };
    if let Some(c) = base_color(tok) {
        return Some(format!("{}", fg_base + c as u32));
    }
    for prefix in ["bright_", "light_", "bright-", "light-"] {
        if let Some(rest) = tok.strip_prefix(prefix) {
            if let Some(c) = base_color(rest) {
                return Some(format!("{}", bright_base + c as u32));
            }
        }
    }
    if tok == "orange" {
        return Some(format!("{};5;208", ext));
    }
    if tok.starts_with('#') {
        let (r, g, b) = rgb_of(tok)?;
        return Some(format!("{};2;{};{};{}", ext, r, g, b));
    }
    if let Ok(n) = tok.parse::<u16>() {
        if n <= 255 {
            return Some(format!("{};5;{}", ext, n));
        }
    }
    None
}

/// Turn a color description into SGR parameters (no ESC[ / m).
///
/// Combine parts with spaces or `+`:  "bold red", "italic+#ff8800", "white bg:blue".
/// Parts: black..white, bright_<name>, orange, 0-255 (256-color index), #rrggbb, bg:<color>,
/// bold dim italic underline blink reverse hidden strike, sgr:<raw params> or raw "1;36".
/// `default` / `auto` / empty mean "no color" and return None.
pub fn color_params(spec: &str) -> Option<String> {
    let s = spec.trim().to_lowercase();
    if s.is_empty() || s == "default" || s == "auto" {
        return None;
    }
    let mut parts: Vec<String> = vec![];
    for tok in s.split(|c: char| c.is_whitespace() || c == '+').filter(|t| !t.is_empty()) {
        let p = if let Some(raw) = tok.strip_prefix("sgr:") {
            Some(raw.to_string())
        } else if tok.contains(';') && tok.chars().all(|c| c.is_ascii_digit() || c == ';') {
            Some(tok.to_string())
        } else if let Some(bg) = tok.strip_prefix("bg:") {
            single_color(bg, true)
        } else {
            match tok {
                "bold" => Some("1".into()),
                "dim" => Some("2".into()),
                "italic" => Some("3".into()),
                "underline" => Some("4".into()),
                "blink" => Some("5".into()),
                "reverse" | "inverse" => Some("7".into()),
                "hidden" => Some("8".into()),
                "strike" | "strikethrough" => Some("9".into()),
                _ => single_color(tok, false),
            }
        };
        parts.push(p?);
    }
    if parts.is_empty() { None } else { Some(parts.join(";")) }
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for d in chars.by_ref() {
                if ('@'..='~').contains(&d) {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

pub fn char_width(c: char) -> usize {
    let u = c as u32;
    if u < 0x300 {
        return 1;
    }
    if (0x300..=0x36F).contains(&u) || (0x200B..=0x200F).contains(&u) || (0xFE00..=0xFE0F).contains(&u) {
        return 0;
    }
    let wide = (0x1100..=0x115F).contains(&u)
        || (0x2E80..=0xA4CF).contains(&u)
        || (0xAC00..=0xD7A3).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFE30..=0xFE6F).contains(&u)
        || (0xFF00..=0xFF60).contains(&u)
        || (0xFFE0..=0xFFE6).contains(&u)
        || (0x1F300..=0x1F64F).contains(&u)
        || (0x1F900..=0x1F9FF).contains(&u)
        || (0x20000..=0x3FFFD).contains(&u);
    if wide { 2 } else { 1 }
}

/// Terminal columns a string occupies (ANSI sequences ignored, wide chars count 2).
pub fn display_width(s: &str) -> usize {
    if !s.contains('\x1b') {
        return s.chars().map(char_width).sum();
    }
    strip_ansi(s).chars().map(char_width).sum()
}

pub fn fmt_bytes(b: u64) -> String {
    let dec = DECIMALS.load(Relaxed) as usize;
    let (base, units): (f64, [&str; 5]) = if BINARY.load(Relaxed) {
        (1024.0, ["B", "KiB", "MiB", "GiB", "TiB"])
    } else {
        (1000.0, ["B", "kB", "MB", "GB", "TB"])
    };
    if (b as f64) < base {
        return format!("{} B", b);
    }
    let mut v = b as f64;
    let mut i = 0;
    while v >= base && i < units.len() - 1 {
        v /= base;
        i += 1;
    }
    // Round half away from zero, like Fastfetch (Rust's {:.N} rounds half to even).
    let m = 10f64.powi(dec as i32);
    format!("{:.*} {}", dec, (v * m).round() / m, units[i])
}

/// Read a small text file and trim it.
pub fn read(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

pub fn read_nonempty(path: &str) -> Option<String> {
    read(path).filter(|s| !s.is_empty())
}

/// Read at most `max` bytes (lossy UTF-8). Avoids slurping huge files like /proc/cpuinfo.
pub fn read_prefix(path: &str, max: usize) -> Option<String> {
    use std::io::Read;
    let mut f = fs::File::open(path).ok()?;
    let mut buf = vec![0u8; max];
    let mut n = 0;
    while n < max {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(_) => break,
        }
    }
    buf.truncate(n);
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Read-only memory map of a file. Pages are loaded on demand, so scanning a big file does not
/// allocate heap memory.
pub struct Mmap {
    ptr: *mut libc::c_void,
    len: usize,
}

unsafe impl Send for Mmap {}

impl Mmap {
    pub fn open(path: &str) -> Option<Mmap> {
        use std::os::fd::AsRawFd;
        let f = fs::File::open(path).ok()?;
        let len = f.metadata().ok()?.len() as usize;
        if len == 0 {
            return None;
        }
        let ptr = unsafe { libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_PRIVATE, f.as_raw_fd(), 0) };
        if ptr == libc::MAP_FAILED {
            return None;
        }
        Some(Mmap { ptr, len })
    }
}

impl std::ops::Deref for Mmap {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr, self.len);
        }
    }
}

// ---------------------------------------------------------------- tiny disk cache

fn cache_dir() -> Option<String> {
    let base = std::env::var("XDG_CACHE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{}/.cache", h)))?;
    Some(format!("{}/nyfetch", base))
}

/// Cached value for `key`, valid only while `stamp` matches what it was stored with.
pub fn cache_get(key: &str, stamp: &str) -> Option<String> {
    if !CACHE.load(Relaxed) {
        return None;
    }
    let text = fs::read_to_string(format!("{}/{}", cache_dir()?, key)).ok()?;
    let (first, rest) = text.split_once('\n')?;
    (first == stamp).then(|| rest.to_string())
}

pub fn cache_put(key: &str, stamp: &str, value: &str) {
    if !CACHE.load(Relaxed) {
        return;
    }
    if let Some(dir) = cache_dir() {
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(format!("{}/{}", dir, key), format!("{}\n{}", stamp, value));
    }
}

/// File identity string (mtime + size) used as a cache stamp.
pub fn file_stamp(path: &str) -> Option<String> {
    use std::time::UNIX_EPOCH;
    let m = fs::metadata(path).ok()?;
    let secs = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(format!("{}:{}:{}", path, secs, m.len()))
}

// ---------------------------------------------------------------- percentages

/// Format a percentage, colored by the configured thresholds. `invert` flips the meaning (battery).
pub fn color_percent(p: f64, invert: bool) -> String {
    let text = format!("{:.0}%", p);
    if !PCT_COLOR.load(Relaxed) {
        return text;
    }
    let warn = PCT_WARN.load(Relaxed) as f64;
    let crit = PCT_CRIT.load(Relaxed) as f64;
    // Bright yellow / bright red, as Fastfetch does.
    let code = if invert {
        let (lo, hi) = (100.0 - crit, 100.0 - warn);
        if p > hi { "32" } else if p > lo { "93" } else { "91" }
    } else if p < warn {
        "32"
    } else if p < crit {
        "93"
    } else {
        "91"
    };
    format!("{}{}{}", sgr(code), text, reset())
}
