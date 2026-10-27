use crate::util::*;
use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::sync::OnceLock;

/// One line of output from a module. `key` and `fmt` are templates using {field} placeholders.
pub struct Entry {
    pub key: String,
    pub fmt: String,
    pub fields: Vec<(String, String)>,
}

fn entry(key: &str, fmt: &str, fields: Vec<(&str, String)>) -> Entry {
    Entry {
        key: key.to_string(),
        fmt: fmt.to_string(),
        fields: fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    }
}

// ---------------------------------------------------------------- basics

pub struct Uname {
    pub sysname: String,
    pub nodename: String,
    pub release: String,
    pub machine: String,
}

pub fn uname() -> &'static Uname {
    static U: OnceLock<Uname> = OnceLock::new();
    U.get_or_init(|| unsafe {
        let mut u: libc::utsname = std::mem::zeroed();
        libc::uname(&mut u);
        let s = |p: *const libc::c_char| CStr::from_ptr(p).to_string_lossy().into_owned();
        Uname {
            sysname: s(u.sysname.as_ptr()),
            nodename: s(u.nodename.as_ptr()),
            release: s(u.release.as_ptr()),
            machine: s(u.machine.as_ptr()),
        }
    })
}

pub fn username() -> String {
    if let Ok(u) = std::env::var("USER") {
        if !u.is_empty() {
            return u;
        }
    }
    unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if !pw.is_null() {
            return CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned();
        }
    }
    "user".into()
}

pub fn hostname() -> String {
    read_nonempty("/proc/sys/kernel/hostname").unwrap_or_else(|| uname().nodename.clone())
}

/// Parsed /etc/os-release, read once and shared between modules and logo detection.
pub fn os_release() -> &'static HashMap<String, String> {
    static CACHE: OnceLock<HashMap<String, String>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let text = fs::read_to_string("/etc/os-release")
            .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
            .unwrap_or_default();
        let mut map = HashMap::new();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                map.insert(k.trim().to_string(), v.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }
        map
    })
}

fn plural(n: u64, unit: &str) -> String {
    if n == 1 { format!("{} {}", n, unit) } else { format!("{} {}s", n, unit) }
}

fn is_junk(s: &str) -> bool {
    let l = s.trim().to_lowercase();
    l.is_empty()
        || l == "default string"
        || l == "none"
        || l == "n/a"
        || l == "unknown"
        || l.contains("to be filled")
        || l.contains("system product name")
        || l.contains("system version")
        || l.contains("o.e.m")
        || l.contains("not specified")
        || l.contains("not applicable")
}

fn dmi(name: &str) -> Option<String> {
    read_nonempty(&format!("/sys/devices/virtual/dmi/id/{}", name)).filter(|s| !is_junk(s))
}

fn extract_version(line: &str) -> Option<String> {
    for word in line.split_whitespace() {
        let w = word.trim_matches(|c: char| c == ',' || c == '(' || c == ')');
        if w.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            let v: String = w.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if v.contains('.') || !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

// ---------------------------------------------------------------- modules

pub fn os() -> Vec<Entry> {
    let rel = os_release();
    let get = |k: &str| rel.get(k).map(|s| s.as_str()).unwrap_or("");
    let id = get("ID");
    // Same per-distro "pretty name" rules as Fastfetch.
    let mut name = String::new();
    if id == "ubuntu" && !get("VERSION").is_empty() && !get("NAME").is_empty() {
        let v = get("VERSION");
        if !v.ends_with(" (development branch)") {
            name = format!("{} {}", get("NAME"), v);
        }
    } else if id == "debian" && get("ID_LIKE").is_empty() && !get("NAME").is_empty() {
        if let Some(dv) = read_nonempty("/etc/debian_version") {
            let codename = get("VERSION_CODENAME");
            name = if codename.is_empty() {
                format!("{} {}", get("NAME"), dv)
            } else {
                format!("{} {} ({})", get("NAME"), dv, codename)
            };
        }
    }
    if name.is_empty() {
        name = if !get("PRETTY_NAME").is_empty() {
            get("PRETTY_NAME").to_string()
        } else if !get("NAME").is_empty() {
            get("NAME").to_string()
        } else {
            "Linux".to_string()
        };
    }
    let arch = uname().machine.clone();
    vec![entry(
        "OS",
        "{name} {arch}",
        vec![
            ("name", name),
            ("arch", arch),
            ("id", rel.get("ID").cloned().unwrap_or_default()),
            ("version", rel.get("VERSION_ID").cloned().unwrap_or_default()),
            ("codename", rel.get("VERSION_CODENAME").cloned().unwrap_or_default()),
        ],
    )]
}

pub fn host() -> Vec<Entry> {
    let name = dmi("product_name");
    let version = dmi("product_version");
    let vendor = dmi("sys_vendor");
    let model = if let Some(n) = name.clone() {
        n
    } else if let Some(m) = read_nonempty("/proc/device-tree/model") {
        m.trim_end_matches('\0').to_string()
    } else {
        return vec![];
    };
    let fmt = if version.is_some() { "{name} ({version})" } else { "{name}" };
    vec![entry(
        "Host",
        fmt,
        vec![
            ("name", model),
            ("version", version.unwrap_or_default()),
            ("vendor", vendor.unwrap_or_default()),
        ],
    )]
}

pub fn kernel() -> Vec<Entry> {
    let u = uname();
    vec![entry(
        "Kernel",
        "{sysname} {release}",
        vec![("sysname", u.sysname.clone()), ("release", u.release.clone()), ("arch", u.machine.clone())],
    )]
}

pub fn uptime() -> Vec<Entry> {
    let secs = match read("/proc/uptime").and_then(|s| s.split_whitespace().next().map(|x| x.to_string())) {
        Some(s) => s.parse::<f64>().unwrap_or(0.0) as u64,
        None => return vec![],
    };
    let (d, h, m, s) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60, secs % 60);
    let mut parts = vec![];
    if d > 0 {
        parts.push(plural(d, "day"));
    }
    if h > 0 {
        parts.push(plural(h, "hour"));
    }
    if m > 0 {
        parts.push(plural(m, "min"));
    }
    if parts.is_empty() {
        parts.push(plural(s, "sec"));
    }
    vec![entry(
        "Uptime",
        "{text}",
        vec![
            ("text", parts.join(", ")),
            ("days", d.to_string()),
            ("hours", h.to_string()),
            ("minutes", m.to_string()),
            ("seconds", s.to_string()),
            ("total_seconds", secs.to_string()),
        ],
    )]
}

fn count_dirs(path: &str) -> usize {
    // file_type() comes from the directory entry itself on most filesystems: no stat per entry.
    fs::read_dir(path)
        .map(|d| d.filter_map(|e| e.ok()).filter(|e| e.file_type().map_or(false, |t| t.is_dir())).count())
        .unwrap_or(0)
}

/// Run `count` unless a cached number for this `stamp` exists. Package databases change rarely,
/// so counting thousands of entries on every run is wasted work.
fn cached_count(key: &str, stamp: Option<String>, count: impl FnOnce() -> usize) -> usize {
    if let Some(s) = &stamp {
        if let Some(n) = cache_get(key, s).and_then(|v| v.trim().parse().ok()) {
            return n;
        }
    }
    let n = count();
    if let Some(s) = &stamp {
        cache_put(key, s, &n.to_string());
    }
    n
}

fn is_valid_nix_pkg(path: &str) -> bool {
    if !std::path::Path::new(path).is_dir() {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.starts_with("nixos-system-nixos-")
        || ["-doc", "-man", "-info", "-dev", "-bin"].iter().any(|s| name.ends_with(s))
    {
        return false;
    }
    // Needs a version-looking part: digits '.' digit.
    let b = name.as_bytes();
    (0..b.len()).any(|i| b[i].is_ascii_digit() && {
        let mut j = i;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        j + 1 < b.len() && b[j] == b'.' && b[j + 1].is_ascii_digit()
    })
}

fn nix_count(path: &str) -> usize {
    // Store paths are immutable, so the resolved profile path is a perfect cache key.
    let Ok(real) = fs::canonicalize(path) else { return 0 };
    let stamp = real.to_string_lossy().into_owned();
    let key = format!("nix_{}", path.replace('/', "_"));
    cached_count(&key, Some(stamp), || {
        let out = Command::new("nix-store").args(["--query", "--requisites", path]).output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).lines().filter(|l| is_valid_nix_pkg(l)).count(),
            Err(_) => 0,
        }
    })
}

fn nix_counts(home: &str) -> Vec<(&'static str, usize)> {
    let mut v = vec![];
    if !std::path::Path::new("/nix").is_dir() && !std::path::Path::new("/run/current-system").exists() {
        return v;
    }
    let state = std::env::var("XDG_STATE_HOME").ok().filter(|s| !s.is_empty()).unwrap_or(format!("{}/.local/state", home));
    let user = std::env::var("USER").unwrap_or_default();
    let mut user_total = 0;
    for p in [
        format!("{}/.nix-profile", home),
        format!("{}/nix/profile", state),
        format!("/etc/profiles/per-user/{}", user),
    ] {
        if std::path::Path::new(&p).exists() {
            user_total += nix_count(&p);
        }
    }
    v.push(("nix-user", user_total));
    v.push(("nix-default", nix_count("/nix/var/nix/profiles/default")));
    v.push(("nix-system", nix_count("/run/current-system")));
    v
}

pub fn packages() -> Vec<Entry> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut found: Vec<(&str, usize)> = vec![];

    if std::path::Path::new("/var/lib/pacman/local").is_dir() {
        let n = cached_count("pkgs_pacman", file_stamp("/var/lib/pacman/local"), || count_dirs("/var/lib/pacman/local"));
        found.push(("pacman", n));
    }
    if std::path::Path::new("/var/lib/dpkg/info").is_dir() {
        // Every installed package owns exactly one <name>.list file; counting directory entries
        // is far cheaper than parsing the multi-megabyte status database. dpkg rewrites `status`
        // on every change, which is what invalidates the cached number.
        let n = cached_count("pkgs_dpkg", file_stamp("/var/lib/dpkg/status"), || {
            fs::read_dir("/var/lib/dpkg/info")
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| e.file_name().as_encoded_bytes().ends_with(b".list"))
                        .count()
                })
                .unwrap_or(0)
        });
        found.push(("dpkg", n));
    } else if let Ok(f) = fs::File::open("/var/lib/dpkg/status") {
        // Fallback: stream the status file through one reusable buffer.
        let mut rd = BufReader::with_capacity(64 * 1024, f);
        let mut line = Vec::with_capacity(256);
        let mut n = 0usize;
        while rd.read_until(b'\n', &mut line).map(|r| r > 0).unwrap_or(false) {
            if line.starts_with(b"Status: install ok installed") {
                n += 1;
            }
            line.clear();
        }
        found.push(("dpkg", n));
    }
    if let Ok(f) = fs::File::open("/lib/apk/db/installed") {
        let n = BufReader::with_capacity(64 * 1024, f)
            .split(b'\n')
            .filter_map(|l| l.ok())
            .filter(|l| l.starts_with(b"P:"))
            .count();
        found.push(("apk", n));
    }
    let rpm_db = ["/var/lib/rpm/rpmdb.sqlite", "/var/lib/rpm/Packages"]
        .iter()
        .find_map(|p| file_stamp(p));
    if let Some(stamp) = rpm_db {
        // `rpm -qa` is slow (tens of ms); the count only changes when the database file does.
        let cached = cache_get("rpm_count", &stamp).and_then(|s| s.trim().parse::<usize>().ok());
        let n = cached.or_else(|| {
            let out = Command::new("rpm").args(["-qa", "--nodigest", "--nosignature"]).output().ok()?;
            let n = out.stdout.iter().filter(|&&b| b == b'\n').count();
            cache_put("rpm_count", &stamp, &n.to_string());
            Some(n)
        });
        if let Some(n) = n.filter(|&n| n > 0) {
            found.push(("rpm", n));
        }
    }
    let flatpak = count_dirs("/var/lib/flatpak/app") + count_dirs(&format!("{}/.local/share/flatpak/app", home));
    if flatpak > 0 {
        found.push(("flatpak", flatpak));
    }
    if let Ok(rd) = fs::read_dir("/var/lib/snapd/snaps") {
        let n = rd
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".snap") && !n.starts_with("core") && !n.starts_with("bare") && !n.starts_with("snapd"))
            .count();
        if n > 0 {
            found.push(("snap", n));
        }
    }
    let nix = nix_counts(&home);
    for (label, n) in nix.iter() {
        found.push((label, *n));
    }
    found.retain(|(_, n)| *n > 0);
    // Fastfetch lists managers alphabetically; nix variants keep their own order (user, default, system).
    found.sort_by_key(|(m, _)| m.split('-').next().unwrap_or(m).to_string());
    if found.is_empty() {
        return vec![];
    }
    let all = found
        .iter()
        .map(|(m, n)| format!("{} ({})", n, m))
        .collect::<Vec<_>>()
        .join(", ");
    let total: usize = found.iter().map(|(_, n)| n).sum();
    let mut fields: Vec<(&str, String)> = vec![("all", all), ("total", total.to_string())];
    for (m, n) in &found {
        fields.push((m, n.to_string()));
    }
    vec![entry("Packages", "{all}", fields)]
}

pub fn shell() -> Vec<Entry> {
    let path = match std::env::var("SHELL") {
        Ok(p) if !p.is_empty() => p,
        _ => return vec![],
    };
    let name = path.rsplit('/').next().unwrap_or(&path).to_string();
    // Starting the shell just to read its version costs milliseconds, so remember the answer
    // until the shell binary itself changes.
    let stamp = file_stamp(&path);
    let version = match stamp.as_deref().and_then(|s| cache_get("shell_version", s)) {
        Some(v) => v.trim().to_string(),
        None => {
            let v = Command::new(&path)
                .arg("--version")
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(|l| l.to_string()))
                .and_then(|l| extract_version(&l))
                .unwrap_or_default();
            if let Some(s) = &stamp {
                cache_put("shell_version", s, &v);
            }
            v
        }
    };
    let fmt = if version.is_empty() { "{name}" } else { "{name} {version}" };
    vec![entry("Shell", fmt, vec![("name", name), ("version", version), ("path", path)])]
}

fn edid_refresh(path: &str, w: u32, h: u32) -> Option<u32> {
    let e = fs::read(path).ok()?;
    if e.len() < 72 {
        return None;
    }
    let clock = (e[54] as u32 | (e[55] as u32) << 8) * 10_000;
    if clock == 0 {
        return None;
    }
    let ha = e[56] as u32 | ((e[58] as u32 & 0xF0) << 4);
    let hb = e[57] as u32 | ((e[58] as u32 & 0x0F) << 8);
    let va = e[59] as u32 | ((e[61] as u32 & 0xF0) << 4);
    let vb = e[60] as u32 | ((e[61] as u32 & 0x0F) << 8);
    if ha != w || va != h {
        return None;
    }
    let total = (ha + hb) * (va + vb);
    if total == 0 {
        return None;
    }
    Some(((clock as f64) / (total as f64)).round() as u32)
}

fn find_in_path(cmd: &str) -> Option<String> {
    std::env::var("PATH").ok()?.split(':').map(|d| format!("{}/{}", d, cmd)).find(|p| std::path::Path::new(p).is_file())
}

/// stdout of `<cmd> <args>`, remembered until the program file itself changes.
fn exe_output_cached(cmd: &str, args: &[&str]) -> Option<String> {
    let exe = find_in_path(cmd)?;
    let stamp = file_stamp(&exe)?;
    let key = format!("exe_{}", cmd);
    if let Some(v) = cache_get(&key, &stamp) {
        return Some(v).filter(|s| !s.is_empty());
    }
    let out = Command::new(&exe).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    cache_put(&key, &stamp, &text);
    Some(text).filter(|s| !s.is_empty())
}

/// Monitor name from EDID: the model-name descriptor, else "<3-letter vendor><4 hex digits>" (e.g. CMN1521).
fn edid_name(e: &[u8]) -> Option<String> {
    if e.len() < 128 {
        return None;
    }
    for i in (0x36..0x7E).step_by(0x12) {
        if e[i] == 0 && e[i + 3] == 0xfc {
            let name: Vec<u8> = e[i + 5..i + 18].iter().cloned().take_while(|&b| b != 0x0a).collect();
            let name = String::from_utf8_lossy(&name).trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    let id = ((e[8] as u16) << 8) | e[9] as u16;
    let letter = |n: u16| (b'A' - 1 + (n & 0x1f) as u8) as char;
    let product = (e[10] as u16) | ((e[11] as u16) << 8);
    Some(format!("{}{}{}{:04X}", letter(id >> 10), letter(id >> 5), letter(id), product))
}

/// Physical panel size in millimetres from EDID.
fn edid_size_mm(e: &[u8]) -> Option<(u32, u32)> {
    if e.len() < 128 {
        return None;
    }
    let dw = (((e[68] as u32) & 0xF0) << 4) + e[66] as u32;
    let dh = (((e[68] as u32) & 0x0F) << 8) + e[67] as u32;
    let (bw, bh) = (e[21] as u32 * 10, e[22] as u32 * 10);
    let (w, h) = if (dw as i64 - bw as i64).abs() < 10 && (dh as i64 - bh as i64).abs() < 10 { (dw, dh) } else { (bw, bh) };
    (w > 0 && h > 0).then_some((w, h))
}

pub fn display() -> Vec<Entry> {
    let mut out = vec![];
    let mut dirs: Vec<_> = match fs::read_dir("/sys/class/drm") {
        Ok(d) => d.filter_map(|e| e.ok()).collect(),
        Err(_) => return out,
    };
    dirs.sort_by_key(|e| e.file_name());
    for d in dirs {
        let fname = d.file_name().to_string_lossy().into_owned();
        let Some((_, conn)) = fname.split_once('-') else { continue };
        if !fname.starts_with("card") {
            continue;
        }
        let base = d.path();
        if read(&format!("{}/status", base.display())).as_deref() != Some("connected") {
            continue;
        }
        let Some(modes) = read(&format!("{}/modes", base.display())) else { continue };
        let Some(first) = modes.lines().next() else { continue };
        let Some((w, h)) = first.split_once('x') else { continue };
        let (Ok(wn), Ok(hn)) = (w.parse::<u32>(), h.trim_end_matches('i').parse::<u32>()) else { continue };
        let edid_path = format!("{}/edid", base.display());
        let refresh = edid_refresh(&edid_path, wn, hn);
        let edid = fs::read(&edid_path).unwrap_or_default();
        let name = edid_name(&edid).unwrap_or_else(|| conn.to_string());
        let inches = edid_size_mm(&edid)
            .map(|(w, h)| ((w * w + h * h) as f64).sqrt() / 25.4)
            .filter(|i| *i > 1.0)
            .map(|i| format!("{}", (i + 0.5) as u32));
        let builtin = ["eDP", "LVDS", "DSI"].iter().any(|p| conn.starts_with(p));
        let ty = if builtin { "Built-in" } else { "External" };
        // "1920x1080 in 15", 144 Hz [Built-in]"
        let mut text = format!("{}x{}", wn, hn);
        if let Some(i) = &inches {
            text.push_str(&format!(" in {}\"", i));
        }
        if let Some(r) = refresh {
            text.push_str(&format!(", {} Hz", r));
        }
        text.push_str(&format!(" [{}]", ty));
        out.push(entry(
            "Display ({name})",
            "{text}",
            vec![
                ("name", name),
                ("connector", conn.to_string()),
                ("width", wn.to_string()),
                ("height", hn.to_string()),
                ("refresh", refresh.map(|r| r.to_string()).unwrap_or_default()),
                ("inches", inches.unwrap_or_default()),
                ("type", ty.to_string()),
                ("text", text),
            ],
        ));
    }
    out
}

pub fn de() -> Vec<Entry> {
    let raw = std::env::var("XDG_CURRENT_DESKTOP").or_else(|_| std::env::var("DESKTOP_SESSION")).unwrap_or_default();
    let first = raw.split(':').next().unwrap_or("").trim().to_string();
    if first.is_empty() {
        return vec![];
    }
    let name = match first.to_lowercase().as_str() {
        "kde" => "KDE Plasma".to_string(),
        "x-cinnamon" => "Cinnamon".to_string(),
        "ubuntu" | "ubuntu:gnome" => "GNOME".to_string(),
        _ => first,
    };
    // Tiling-WM-only sessions are not a DE.
    let l = name.to_lowercase();
    if ["hyprland", "sway", "i3", "bspwm", "river", "niri", "dwm", "awesome"].contains(&l.as_str()) {
        return vec![];
    }
    vec![entry("Desktop Environment", "{name}", vec![("name", name)])]
}

fn wm_from_env() -> Option<String> {
    let has = |k: &str| std::env::var_os(k).map_or(false, |v| !v.is_empty());
    if has("HYPRLAND_INSTANCE_SIGNATURE") {
        return Some("Hyprland".into());
    }
    if has("SWAYSOCK") {
        return Some("Sway".into());
    }
    if has("NIRI_SOCKET") {
        return Some("niri".into());
    }
    if has("I3SOCK") {
        return Some("i3".into());
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default()
        .to_lowercase();
    for token in desktop.split(':') {
        let wm = match token.trim() {
            "gnome" | "ubuntu" | "unity" | "pop" => "Mutter",
            "kde" | "plasma" | "plasmax11" => "KWin",
            "xfce" | "xfce4" => "xfwm4",
            "x-cinnamon" | "cinnamon" => "Muffin",
            "mate" => "Marco",
            "cosmic" => "cosmic-comp",
            "hyprland" => "Hyprland",
            "sway" => "Sway",
            "river" => "river",
            "labwc" => "labwc",
            "wayfire" => "Wayfire",
            "bspwm" => "bspwm",
            "openbox" => "Openbox",
            "awesome" => "awesome",
            "qtile" => "Qtile",
            "dwm" => "dwm",
            "i3" => "i3",
            _ => continue,
        };
        return Some(wm.to_string());
    }
    None
}

/// Version of the running window manager, for the few that are easy to ask. Empty when unknown.
fn wm_version(name: &str) -> String {
    let v = match name {
        "niri" => exe_output_cached("niri", &["--version"]).map(|o| {
            // "niri 25.11 (commit b35bcae)"
            let rest = o.split_once(' ').map(|x| x.1).unwrap_or("");
            rest.rsplit_once('(').map(|x| x.0).unwrap_or(rest).trim().to_string()
        }),
        "Sway" | "sway" => exe_output_cached("sway", &["--version"])
            .and_then(|o| o.split_once("version ").map(|x| x.1.split_whitespace().next().unwrap_or("").to_string())),
        "i3" => exe_output_cached("i3", &["--version"])
            .and_then(|o| o.split_once("version ").map(|x| x.1.split_whitespace().next().unwrap_or("").to_string())),
        "labwc" => exe_output_cached("labwc", &["--version"]).and_then(|o| o.split_whitespace().nth(1).map(|s| s.to_string())),
        "Hyprland" => fs::read_to_string("/usr/include/hyprland/src/version.h")
            .ok()
            .and_then(|t| {
                let l = t.lines().find(|l| l.starts_with("#define GIT_TAG "))?;
                Some(l.split('"').nth(1)?.trim_start_matches('v').to_string())
            })
            .or_else(|| exe_output_cached("Hyprland", &["--version"]).and_then(|o| o.split_whitespace().nth(1).map(|s| s.trim_start_matches('v').to_string()))),
        _ => None,
    };
    v.unwrap_or_default()
}

pub fn wm() -> Vec<Entry> {
    const KNOWN: &[(&str, &str)] = &[
        ("Hyprland", "Hyprland"),
        ("sway", "Sway"),
        ("kwin_wayland", "KWin"),
        ("kwin_x11", "KWin"),
        ("gnome-shell", "Mutter"),
        ("mutter", "Mutter"),
        ("i3", "i3"),
        ("bspwm", "bspwm"),
        ("openbox", "Openbox"),
        ("xfwm4", "xfwm4"),
        ("awesome", "awesome"),
        ("dwm", "dwm"),
        ("river", "river"),
        ("niri", "niri"),
        ("labwc", "labwc"),
        ("wayfire", "Wayfire"),
        ("qtile", "Qtile"),
        ("herbstluftwm", "herbstluftwm"),
        ("xmonad", "XMonad"),
        ("marco", "Marco"),
        ("muffin", "Muffin"),
        ("cinnamon", "Muffin"),
        ("weston", "Weston"),
        ("cage", "Cage"),
        ("icewm", "IceWM"),
        ("fluxbox", "Fluxbox"),
        ("budgie-wm", "Budgie"),
        ("cosmic-comp", "cosmic-comp"),
    ];
    // Fast path: most sessions identify their WM through the environment, no /proc scan needed.
    let mut name = wm_from_env();
    // Without any graphical session variable there is no window manager to find, so skip the scan.
    let graphical = ["WAYLAND_DISPLAY", "DISPLAY", "XDG_SESSION_TYPE"]
        .iter()
        .any(|k| std::env::var_os(k).map_or(false, |v| !v.is_empty() && v != "tty"));
    if name.is_none() && graphical {
        let mut comms = HashSet::new();
        if let Ok(rd) = fs::read_dir("/proc") {
            for e in rd.filter_map(|e| e.ok()) {
                let n = e.file_name();
                let n = n.to_string_lossy();
                if n.as_bytes().first().map_or(false, |b| b.is_ascii_digit()) {
                    if let Some(c) = read(&format!("/proc/{}/comm", n)) {
                        comms.insert(c);
                    }
                }
            }
        }
        name = KNOWN.iter().find(|(c, _)| comms.contains(*c)).map(|(_, n)| n.to_string());
    }
    let Some(name) = name else { return vec![] };
    let protocol = match std::env::var("XDG_SESSION_TYPE").unwrap_or_default().as_str() {
        "wayland" => "Wayland",
        "x11" => "X11",
        "tty" => "TTY",
        _ => "",
    }
    .to_string();
    let version = wm_version(&name);
    let fmt = match (version.is_empty(), protocol.is_empty()) {
        (true, true) => "{name}",
        (true, false) => "{name} ({protocol})",
        (false, true) => "{name} {version}",
        (false, false) => "{name} {version} ({protocol})",
    };
    vec![entry("Window Manager", fmt, vec![("name", name), ("version", version), ("protocol", protocol)])]
}

fn gtk_setting(key: &str) -> Option<(String, &'static str)> {
    let home = std::env::var("HOME").ok()?;
    let cfg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home));
    for (dir, tag) in [("gtk-3.0", "GTK3"), ("gtk-4.0", "GTK4")] {
        if let Ok(text) = fs::read_to_string(format!("{}/{}/settings.ini", cfg, dir)) {
            for line in text.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    if k.trim() == key {
                        let v = v.trim().trim_matches('"').to_string();
                        if !v.is_empty() {
                            return Some((v, tag));
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn theme() -> Vec<Entry> {
    match gtk_setting("gtk-theme-name") {
        Some((n, t)) => vec![entry("Theme", "{name} [{toolkit}]", vec![("name", n), ("toolkit", t.into())])],
        None => vec![],
    }
}

pub fn icons() -> Vec<Entry> {
    match gtk_setting("gtk-icon-theme-name") {
        Some((n, t)) => vec![entry("Icons", "{name} [{toolkit}]", vec![("name", n), ("toolkit", t.into())])],
        None => vec![],
    }
}

pub fn font() -> Vec<Entry> {
    match gtk_setting("gtk-font-name") {
        Some((raw, t)) => {
            let (name, size) = match raw.rsplit_once(' ') {
                Some((n, s)) if s.chars().all(|c| c.is_ascii_digit() || c == '.') => (n.to_string(), s.to_string()),
                _ => (raw.clone(), String::new()),
            };
            let fmt = if size.is_empty() { "{name} [{toolkit}]" } else { "{name} ({size}pt) [{toolkit}]" };
            vec![entry("Font", fmt, vec![("name", name), ("size", size), ("toolkit", t.into())])]
        }
        None => vec![],
    }
}

pub fn cursor() -> Vec<Entry> {
    // Same search order as Fastfetch: GTK settings, XCURSOR_* variables, ~/.icons/default, .Xresources,
    // then the system default theme.
    // Like Fastfetch: no cursor in a plain text console.
    let graphical = ["WAYLAND_DISPLAY", "DISPLAY", "XDG_SESSION_TYPE"]
        .iter()
        .any(|k| std::env::var_os(k).map_or(false, |v| !v.is_empty() && v != "tty"));
    if !graphical {
        return vec![];
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let inherits = |path: String| -> Option<String> {
        fs::read_to_string(path).ok()?.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "Inherits").then(|| v.trim().to_string()).filter(|v| !v.is_empty())
        })
    };
    let (theme, size) = if let Some((n, _)) = gtk_setting("gtk-cursor-theme-name") {
        (n, gtk_setting("gtk-cursor-theme-size").map(|(s, _)| s).unwrap_or_default())
    } else if let Some(t) = env("XCURSOR_THEME") {
        (t, env("XCURSOR_SIZE").unwrap_or_default())
    } else if let Some(t) = inherits(format!("{}/.icons/default/index.theme", home)) {
        (t, env("XCURSOR_SIZE").unwrap_or_default())
    } else if let Some(t) = fs::read_to_string(format!("{}/.Xresources", home)).ok().and_then(|x| {
        x.lines().find_map(|l| l.strip_prefix("Xcursor.theme:").map(|v| v.trim().to_string()))
    }) {
        (t, env("XCURSOR_SIZE").unwrap_or_default())
    } else if let Some(t) = inherits("/usr/share/icons/default/index.theme".into()) {
        (t, env("XCURSOR_SIZE").unwrap_or_default())
    } else {
        return vec![];
    };
    let fmt = if size.is_empty() { "{name}" } else { "{name} ({size}px)" };
    vec![entry("Cursor", fmt, vec![("name", theme), ("size", size)])]
}

fn proc_parent(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

fn proc_name(pid: u32) -> String {
    if let Ok(exe) = fs::read_link(format!("/proc/{}/exe", pid)) {
        if let Some(n) = exe.file_name() {
            let n = n.to_string_lossy().into_owned();
            let n = n.trim_start_matches('.').trim_end_matches("-wrapped").to_string();
            if !n.is_empty() {
                return n;
            }
        }
    }
    read(&format!("/proc/{}/comm", pid)).unwrap_or_default()
}

/// Name of the terminal program we run in (the first non-shell ancestor process). Found once.
fn detect_terminal() -> Option<&'static str> {
    static NAME: OnceLock<Option<String>> = OnceLock::new();
    NAME.get_or_init(|| {
        const SKIP: &[&str] = &[
            "bash", "zsh", "fish", "sh", "dash", "ash", "nu", "nushell", "xonsh", "elvish", "tcsh", "csh", "ksh",
            "sudo", "su", "doas", "tmux", "tmux: server", "screen", "nyfetch", "login", "script", "env", "fastfetch",
        ];
        const STOP: &[&str] = &["systemd", "init", "sshd", "login"];
        let mut pid = std::process::id();
        let mut result = None;
        for _ in 0..12 {
            let Some(parent) = proc_parent(pid) else { break };
            if parent <= 1 {
                break;
            }
            let name = proc_name(parent);
            if STOP.contains(&name.as_str()) {
                if name == "sshd" {
                    result = Some("sshd".to_string());
                }
                break;
            }
            if !SKIP.contains(&name.as_str()) && !name.is_empty() {
                result = Some(name);
                break;
            }
            pid = parent;
        }
        let name = result.or_else(|| std::env::var("TERM_PROGRAM").ok()).or_else(|| std::env::var("TERM").ok())?;
        if name.is_empty() {
            return None;
        }
        Some(match name.as_str() {
            "wezterm-gui" => "WezTerm".to_string(),
            "gnome-terminal-" | "gnome-terminal-server" => "gnome-terminal".to_string(),
            _ => name,
        })
    })
    .as_deref()
}

fn terminal_version(name: &str) -> String {
    if name == "kitty" {
        // Same file Fastfetch reads: "version: Version = Version(0, 49, 2)".
        for p in ["/usr/lib64/kitty/kitty/constants.py", "/usr/lib/kitty/kitty/constants.py"] {
            if let Some(t) = read_prefix(p, 1024) {
                if let Some(i) = t.find("version: Version = Version(") {
                    let nums: Vec<&str> = t[i + 27..].split(')').next().unwrap_or("").split(',').map(|x| x.trim()).collect();
                    if nums.len() == 3 && nums.iter().all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) {
                        return nums.join(".");
                    }
                }
            }
        }
        return exe_output_cached("kitty", &["--version"])
            .and_then(|o| o.split_whitespace().nth(1).map(|s| s.to_string()))
            .unwrap_or_default();
    }
    std::env::var("TERM_PROGRAM_VERSION").ok().filter(|v| !v.is_empty()).unwrap_or_default()
}

pub fn terminal() -> Vec<Entry> {
    let Some(name) = detect_terminal() else { return vec![] };
    let version = terminal_version(name);
    let fmt = if version.is_empty() { "{name}" } else { "{name} {version}" };
    vec![entry("Terminal", fmt, vec![("name", name.to_string()), ("version", version)])]
}

/// Ask kitty for its font through the terminal itself (an XTGETTCAP query on /dev/tty). Returns the
/// decoded answers, or None when the terminal does not reply quickly.
fn kitty_query(keys: &[&str]) -> Option<Vec<String>> {
    use std::io::{IsTerminal, Write};
    use std::os::fd::AsRawFd;
    if !std::io::stdout().is_terminal() || std::env::var_os("TMUX").is_some() {
        return None;
    }
    let hex = |s: &str| s.bytes().map(|b| format!("{:02x}", b)).collect::<String>();
    let query = format!("\x1bP+q{}\x1b\\", keys.iter().map(|k| hex(&format!("kitty-query-{}", k))).collect::<Vec<_>>().join(";"));
    let mut tty = fs::OpenOptions::new().read(true).write(true).open("/dev/tty").ok()?;
    let fd = tty.as_raw_fd();
    let mut old: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut old) } != 0 {
        return None;
    }
    let mut raw = old;
    raw.c_lflag &= !(libc::ICANON | libc::ECHO);
    raw.c_cc[libc::VMIN] = 0;
    raw.c_cc[libc::VTIME] = 0;
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) };
    let mut buf: Vec<u8> = vec![];
    if tty.write_all(query.as_bytes()).is_ok() {
        let start = std::time::Instant::now();
        let count = |b: &[u8]| b.windows(2).filter(|w| *w == b"\x1b\\").count();
        while count(&buf) < keys.len() {
            let left = 200i64 - start.elapsed().as_millis() as i64;
            if left <= 0 {
                break;
            }
            let mut p = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
            if unsafe { libc::poll(&mut p, 1, left as i32) } <= 0 {
                break;
            }
            let mut chunk = [0u8; 512];
            let n = unsafe { libc::read(fd, chunk.as_mut_ptr() as *mut _, chunk.len()) };
            if n <= 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n as usize]);
        }
    }
    unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &old) };
    let text = String::from_utf8_lossy(&buf).into_owned();
    let answers: Vec<String> = text
        .split("\x1bP1+r")
        .skip(1)
        .filter_map(|part| {
            let hexv = part.split_once('=')?.1.split('\x1b').next()?;
            let bytes: Vec<u8> = (0..hexv.len() / 2).filter_map(|i| u8::from_str_radix(&hexv[i * 2..i * 2 + 2], 16).ok()).collect();
            Some(String::from_utf8_lossy(&bytes).into_owned())
        })
        .collect();
    (answers.len() == keys.len()).then_some(answers)
}

pub fn terminal_font() -> Vec<Entry> {
    // Only kitty is supported so far.
    if detect_terminal() != Some("kitty") {
        return vec![];
    }
    let (family, size) = match kitty_query(&["font_family", "font_size"]) {
        Some(a) => (a[0].clone(), a[1].clone()),
        None => {
            // No answer from the terminal: read kitty.conf, then use kitty's defaults.
            let home = std::env::var("HOME").unwrap_or_default();
            let cfg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home));
            let text = fs::read_to_string(format!("{}/kitty/kitty.conf", cfg)).unwrap_or_default();
            let get = |key: &str| {
                text.lines().find_map(|l| {
                    let l = l.trim();
                    l.strip_prefix(key).filter(|r| r.starts_with(char::is_whitespace)).map(|r| r.trim().to_string())
                })
            };
            (get("font_family").unwrap_or_else(|| "monospace".into()), get("font_size").unwrap_or_else(|| "11.0".into()))
        }
    };
    let size_text = match size.parse::<f64>() {
        Ok(v) if v.fract() == 0.0 => format!("{}", v as u32),
        Ok(v) => format!("{}", (v * 10.0).round() / 10.0),
        Err(_) => size,
    };
    vec![entry("Terminal Font", "{name} ({size}pt)", vec![("name", family), ("size", size_text)])]
}

fn clean_cpu_name(raw: &str) -> String {
    // Mirrors Fastfetch: drop marketing words, cut at " w/ Radeon" and at '@', keep (R)/(TM).
    let mut n = raw.to_string();
    for junk in [
        " CPU", " FPU", " APU", " Processor", " Dual-Core", " Quad-Core", " Six-Core", " Eight-Core", " Ten-Core",
        " 2-Core", " 4-Core", " 6-Core", " 8-Core", " 10-Core", " 12-Core", " 14-Core", " 16-Core",
    ] {
        n = n.replace(junk, "");
    }
    for cut in [" w/ Radeon ", " with Radeon "] {
        if let Some(i) = n.find(cut) {
            n.truncate(i);
        }
    }
    if let Some(i) = n.find('@') {
        n.truncate(i);
    }
    n.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Number of online CPUs from a list such as "0-7,9,12-15".
fn cpu_list_count(list: &str) -> Option<usize> {
    let mut n = 0;
    for part in list.trim().split(',').filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => n += b.parse::<usize>().ok()?.checked_sub(a.parse::<usize>().ok()?)? + 1,
            None => {
                part.parse::<usize>().ok()?;
                n += 1;
            }
        }
    }
    (n > 0).then_some(n)
}

pub fn cpu() -> Vec<Entry> {
    // Only the first CPU's block is needed for the model name; the core count comes from sysfs,
    // so a 128-thread machine does not make us read and parse 150 kB of /proc/cpuinfo.
    let mut cores = read("/sys/devices/system/cpu/online").and_then(|l| cpu_list_count(&l)).unwrap_or(0);
    let count_processors = cores == 0;
    let mut info = String::new();
    if count_processors {
        // No sysfs CPU list: fall back to counting "processor" lines in the whole file.
        info = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    } else if let Ok(mut f) = fs::File::open("/proc/cpuinfo") {
        use std::io::Read;
        let mut buf = [0u8; 4096];
        if let Ok(n) = f.read(&mut buf) {
            info = String::from_utf8_lossy(&buf[..n]).into_owned();
        }
    }
    let mut name = None;
    let mut mhz = None;
    for line in info.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "model name" | "Model Name" | "cpu model" if name.is_none() => name = Some(v.to_string()),
            "processor" if count_processors => cores += 1,
            "cpu MHz" if mhz.is_none() => mhz = v.parse::<f64>().ok(),
            _ => {}
        }
    }
    let name = name
        .or_else(|| read_nonempty("/proc/device-tree/model").map(|s| s.trim_end_matches('\0').to_string()))
        .unwrap_or_else(|| "Unknown CPU".into());
    let ghz = ["cpuinfo_max_freq", "scaling_max_freq"]
        .iter()
        .find_map(|f| read(&format!("/sys/devices/system/cpu/cpu0/cpufreq/{}", f)))
        .and_then(|s| s.parse::<f64>().ok())
        .map(|khz| khz / 1_000_000.0)
        .or_else(|| mhz.map(|m| m / 1000.0));
    let name = clean_cpu_name(&name);
    let (fmt, freq) = match ghz {
        Some(g) if cores > 1 => ("{name} ({cores}) @ {freq} GHz", format!("{:.2}", g)),
        Some(g) => ("{name} @ {freq} GHz", format!("{:.2}", g)),
        None if cores > 1 => ("{name} ({cores})", String::new()),
        None => ("{name}", String::new()),
    };
    vec![entry("CPU", fmt, vec![("name", name), ("cores", cores.to_string()), ("freq", freq)])]
}

fn pci_lookup(vendor: u16, device: u16) -> (Option<String>, Option<String>) {
    // pci.ids is over a megabyte. Map it instead of reading it: the kernel only pages in the
    // parts we touch, and a SIMD substring search jumps straight to the vendor block.
    let map = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids", "/usr/share/pci.ids"]
        .iter()
        .find_map(|p| Mmap::open(p));
    let Some(map) = map else { return (None, None) };
    let text: &[u8] = &map;
    let text_of = |b: &[u8]| String::from_utf8_lossy(b).trim().to_string();

    let vendor_needle = format!("\n{:04x}  ", vendor);
    let Some(vpos) = memchr::memmem::find(text, vendor_needle.as_bytes()) else { return (None, None) };
    let name_start = vpos + vendor_needle.len();
    let name_end = memchr::memchr(b'\n', &text[name_start..]).map_or(text.len(), |i| name_start + i);
    let vname = Some(text_of(&text[name_start..name_end]));

    let did = format!("{:04x}", device);
    let mut pos = name_end + 1;
    while pos < text.len() {
        let end = memchr::memchr(b'\n', &text[pos..]).map_or(text.len(), |i| pos + i);
        let line = &text[pos..end];
        match line.first() {
            Some(b'\t') if line.get(1) != Some(&b'\t') => {
                if line[1..].starts_with(did.as_bytes()) && line.len() > 7 {
                    return (vname, Some(text_of(&line[5..])));
                }
            }
            Some(b'\t') | Some(b'#') | None => {}
            _ => break, // next vendor
        }
        pos = end + 1;
    }
    (vname, None)
}

pub fn gpu() -> Vec<Entry> {
    let mut cards: Vec<_> = match fs::read_dir("/sys/class/drm") {
        Ok(d) => d
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("card") && !n.contains('-')
            })
            .collect(),
        Err(_) => return vec![],
    };
    cards.sort_by_key(|e| e.file_name());
    let hex = |p: String| read(&p).and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok());
    let mut gpus = vec![];
    for c in cards {
        let base = format!("{}/device", c.path().display());
        let (Some(v), Some(d)) = (hex(format!("{}/vendor", base)), hex(format!("{}/device", base))) else { continue };
        if let Some(class) = hex(format!("{}/class", base)) {
            if class >> 16 != 0x03 {
                continue;
            }
        }
        let (vname, dname) = pci_lookup(v as u16, d as u16);
        let vendor = match v {
            0x1002 => "AMD".to_string(),
            0x10de => "NVIDIA".to_string(),
            0x8086 => "Intel".to_string(),
            _ => vname
                .unwrap_or_default()
                .replace(", Inc.", "")
                .replace(" Corporation", "")
                .replace(" Inc.", ""),
        };
        let raw = dname.unwrap_or_else(|| format!("Device {:04x}", d));
        let name = match (raw.rfind('['), raw.rfind(']')) {
            (Some(a), Some(b)) if b > a => raw[a + 1..b].to_string(),
            _ => raw,
        };
        let vram = read(&format!("{}/mem_info_vram_total", base)).and_then(|s| s.parse::<u64>().ok());
        let ty = match v {
            0x10de => "Discrete",
            0x8086 if name.contains("Arc") => "Discrete",
            0x8086 => "Integrated",
            0x1002 => match vram {
                Some(b) if b >= 2 << 30 => "Discrete",
                Some(_) => "Integrated",
                None => "",
            },
            _ => "",
        };
        // Intel exposes the maximum graphics clock; Fastfetch shows it after the name.
        let freq_mhz = [
            format!("{}/gt_max_freq_mhz", c.path().display()),
            format!("{}/tile0/gt0/freq0/max_freq", base),
        ]
        .iter()
        .find_map(|p| read(p).and_then(|s| s.parse::<u32>().ok()))
        .filter(|f| *f > 0);
        let freq = match freq_mhz {
            Some(f) if f >= 1000 => format!("{:.2} GHz", f as f64 / 1000.0),
            Some(f) => format!("{} MHz", f),
            None => String::new(),
        };
        gpus.push((vendor, name, ty, freq));
    }
    let multi = gpus.len() > 1;
    gpus.into_iter()
        .enumerate()
        .map(|(i, (vendor, name, ty, freq))| {
            let fmt = match (freq.is_empty(), ty.is_empty()) {
                (true, true) => "{vendor} {name}",
                (true, false) => "{vendor} {name} [{type}]",
                (false, true) => "{vendor} {name} @ {freq}",
                (false, false) => "{vendor} {name} @ {freq} [{type}]",
            };
            entry(
                if multi { "GPU {index}" } else { "GPU" },
                fmt,
                vec![("vendor", vendor), ("name", name), ("type", ty.to_string()), ("freq", freq), ("index", (i + 1).to_string())],
            )
        })
        .collect()
}

/// The handful of /proc/meminfo values we use, in bytes. No map, no per-line allocation.
#[derive(Default)]
struct MemInfo {
    total: Option<u64>,
    free: Option<u64>,
    available: Option<u64>,
    swap_total: Option<u64>,
    swap_free: Option<u64>,
}

fn meminfo() -> MemInfo {
    let mut m = MemInfo::default();
    let text = read_prefix("/proc/meminfo", 4096).unwrap_or_default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else { continue };
        let slot = match k {
            "MemTotal" => &mut m.total,
            "MemFree" => &mut m.free,
            "MemAvailable" => &mut m.available,
            "SwapTotal" => &mut m.swap_total,
            "SwapFree" => &mut m.swap_free,
            _ => continue,
        };
        *slot = v.split_whitespace().next().and_then(|x| x.parse::<u64>().ok()).map(|n| n * 1024);
    }
    m
}

pub fn memory() -> Vec<Entry> {
    let m = meminfo();
    let Some(total) = m.total else { return vec![] };
    let avail = m.available.or(m.free).unwrap_or(0);
    let used = total.saturating_sub(avail);
    let pct = used as f64 * 100.0 / total as f64;
    vec![entry(
        "Memory",
        "{used} / {total} ({percent})",
        vec![
            ("used", fmt_bytes(used)),
            ("total", fmt_bytes(total)),
            ("percent", color_percent(pct, false)),
            ("percent_num", format!("{:.0}", pct)),
        ],
    )]
}

pub fn swap() -> Vec<Entry> {
    let m = meminfo();
    let total = m.swap_total.unwrap_or(0);
    if total == 0 {
        return vec![entry("Swap", "Disabled", vec![])];
    }
    let used = total.saturating_sub(m.swap_free.unwrap_or(0));
    let pct = used as f64 * 100.0 / total as f64;
    vec![entry(
        "Swap",
        "{used} / {total} ({percent})",
        vec![
            ("used", fmt_bytes(used)),
            ("total", fmt_bytes(total)),
            ("percent", color_percent(pct, false)),
            ("percent_num", format!("{:.0}", pct)),
        ],
    )]
}

pub fn disk(paths: Option<Vec<String>>) -> Vec<Entry> {
    let mounts = fs::read_to_string("/proc/mounts").unwrap_or_default();
    // (mountpoint, fstype, tags)
    let mut list: Vec<(String, String, String)> = vec![];
    let opts_of = |mp: &str| -> String {
        mounts
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                (f.len() >= 4 && f[1].replace("\\040", " ") == mp).then(|| f[3].to_string())
            })
            .last()
            .unwrap_or_default()
    };
    if let Some(paths) = paths {
        for p in paths {
            let fs_type = mounts
                .lines()
                .filter_map(|l| {
                    let f: Vec<&str> = l.split_whitespace().collect();
                    (f.len() >= 3 && f[1] == p).then(|| f[2].to_string())
                })
                .last()
                .unwrap_or_default();
            let ro = opts_of(&p).split(',').any(|o| o == "ro");
            list.push((p, fs_type, if ro { " [Read-only]".into() } else { String::new() }));
        }
    } else {
        use std::os::unix::fs::FileTypeExt;
        let mut seen = HashSet::new();
        for l in mounts.lines() {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() < 4 {
                continue;
            }
            let (src, fst) = (f[0], f[2]);
            let mp = f[1].replace("\\040", " ");
            // Fastfetch hides the boot partitions by default; list them with `paths = { "/boot/efi" }`.
            if mp == "/efi" || mp == "/boot" || mp.starts_with("/boot/") {
                continue;
            }
            if mp != "/" {
                // Same "physical disk" rules as Fastfetch.
                if src == "none" || mp.starts_with("/bedrock/") {
                    continue;
                }
                if fst == "9p" {
                    if !f[3].split(',').any(|o| o == "aname=drvfs") {
                        continue;
                    }
                } else if fst != "zfs" && fst != "fuse.sshfs" {
                    if !src.starts_with("/dev/")
                        || src.starts_with("/dev/loop")
                        || src.starts_with("/dev/ram")
                        || src.starts_with("/dev/fd")
                    {
                        continue;
                    }
                    if !fs::metadata(src).map(|m| m.file_type().is_block_device()).unwrap_or(false) {
                        continue;
                    }
                }
            }
            // A second mount of the same device is a subvolume (hidden by default).
            if !seen.insert(src.to_string()) {
                continue;
            }
            let mut tags: Vec<&str> = vec![];
            if let Some(name) = src.strip_prefix("/dev/") {
                let base = format!("/sys/class/block/{}", name);
                let removable = fs::canonicalize(&base)
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.join("removable")))
                    .and_then(|p| read(&p.to_string_lossy()))
                    .or_else(|| read(&format!("{}/removable", base)));
                if removable.as_deref() == Some("1") {
                    tags.push("External");
                }
            }
            if f[3].split(',').any(|o| o == "ro") {
                tags.push("Read-only");
            }
            let tag = if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(", ")) };
            list.push((mp, fst.to_string(), tag));
        }
        list.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        if list.is_empty() {
            list.push(("/".into(), String::new(), String::new()));
        }
    }
    let mut out = vec![];
    for (mp, fs_type, tags) in list {
        let Ok(c) = std::ffi::CString::new(mp.clone()) else { continue };
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
            continue;
        }
        let total = s.f_blocks as u64 * s.f_frsize as u64;
        if total == 0 {
            continue;
        }
        let free = s.f_bfree as u64 * s.f_frsize as u64;
        let used = total.saturating_sub(free);
        let pct = used as f64 * 100.0 / total as f64;
        let fmt = if fs_type.is_empty() {
            "{used} / {total} ({percent})"
        } else {
            "{used} / {total} ({percent}) - {fs}{tags}"
        };
        out.push(entry(
            "Disk ({path})",
            fmt,
            vec![
                ("path", mp),
                ("used", fmt_bytes(used)),
                ("total", fmt_bytes(total)),
                ("percent", color_percent(pct, false)),
                ("percent_num", format!("{:.0}", pct)),
                ("fs", fs_type),
                ("tags", tags),
            ],
        ));
    }
    out
}

pub fn localip() -> Vec<Entry> {
    let mut out = vec![];
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return out;
        }
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            cur = ifa.ifa_next;
            if ifa.ifa_addr.is_null() || ifa.ifa_netmask.is_null() {
                continue;
            }
            let flags = ifa.ifa_flags;
            if flags & libc::IFF_LOOPBACK as u32 != 0 || flags & libc::IFF_UP as u32 == 0 {
                continue;
            }
            if (*ifa.ifa_addr).sa_family as i32 != libc::AF_INET {
                continue;
            }
            let addr = &*(ifa.ifa_addr as *const libc::sockaddr_in);
            let mask = &*(ifa.ifa_netmask as *const libc::sockaddr_in);
            let ip = std::net::Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
            let prefix = u32::from_be(mask.sin_addr.s_addr).count_ones();
            let name = CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned();
            out.push(entry(
                "Local IP ({name})",
                "{ip}/{prefix}",
                vec![("name", name), ("ip", ip.to_string()), ("prefix", prefix.to_string())],
            ));
        }
        libc::freeifaddrs(ifap);
    }
    out
}

#[allow(dead_code)]
fn duration_text(secs: u64) -> String {
    let (d, h, m) = (secs / 86400, secs % 86400 / 3600, secs % 3600 / 60);
    let mut parts = vec![];
    if d > 0 {
        parts.push(plural(d, "day"));
    }
    if h > 0 {
        parts.push(plural(h, "hour"));
    }
    if m > 0 || parts.is_empty() {
        parts.push(plural(m, "min"));
    }
    parts.join(", ")
}

pub fn battery() -> Vec<Entry> {
    let mut out = vec![];
    let Ok(rd) = fs::read_dir("/sys/class/power_supply") else { return out };
    let mut dirs: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    dirs.sort_by_key(|e| e.file_name());
    // Is a charger plugged in? (any "Mains" supply that is online, like Fastfetch)
    let ac = dirs.iter().any(|d| {
        let base = d.path().display().to_string();
        read(&format!("{}/type", base)).as_deref() == Some("Mains") && read(&format!("{}/online", base)).as_deref() == Some("1")
    });
    for d in dirs {
        let base = d.path().display().to_string();
        if read(&format!("{}/type", base)).as_deref() != Some("Battery") {
            continue;
        }
        // Batteries of accessories (mouse, keyboard, headset...) have scope "Device": not the computer's.
        if read(&format!("{}/scope", base)).as_deref() == Some("Device") {
            continue;
        }
        let Some(cap) = read(&format!("{}/capacity", base)).and_then(|s| s.parse::<f64>().ok()) else { continue };
        let raw = read(&format!("{}/status", base)).unwrap_or_default();
        let mut parts: Vec<&str> = vec![];
        if ac {
            parts.push("AC Connected");
        }
        match raw.as_str() {
            "Charging" => parts.push("Charging"),
            "Discharging" => parts.push("Discharging"),
            _ => {}
        }
        if read(&format!("{}/capacity_level", base)).as_deref() == Some("Critical") {
            parts.push("Critical");
        }
        if raw == "Unknown" {
            parts.push("Unknown");
        }
        let status = parts.join(", ");
        let model = read_nonempty(&format!("{}/model_name", base)).unwrap_or_else(|| d.file_name().to_string_lossy().into_owned());
        out.push(entry(
            "Battery ({model})",
            if status.is_empty() { "{percent}" } else { "{percent} [{status}]" },
            vec![
                ("model", model),
                ("percent", color_percent(cap, true)),
                ("percent_num", format!("{:.0}", cap)),
                ("status", status),
            ],
        ));
    }
    out
}

pub fn locale() -> Vec<Entry> {
    let v = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|s| !s.is_empty()));
    vec![entry("Locale", "{name}", vec![("name", v.unwrap_or_else(|| "C".into()))])]
}

pub fn board() -> Vec<Entry> {
    let Some(name) = dmi("board_name") else { return vec![] };
    let version = dmi("board_version");
    let vendor = dmi("board_vendor").unwrap_or_default();
    let fmt = if version.is_some() { "{name} ({version})" } else { "{name}" };
    vec![entry("Board", fmt, vec![("name", name), ("version", version.unwrap_or_default()), ("vendor", vendor)])]
}

pub fn bios() -> Vec<Entry> {
    let Some(version) = dmi("bios_version") else { return vec![] };
    let vendor = dmi("bios_vendor").unwrap_or_default();
    let date = dmi("bios_date").unwrap_or_default();
    let ty = if std::path::Path::new("/sys/firmware/efi").exists() { "UEFI" } else { "BIOS" };
    vec![entry(
        "BIOS ({type})",
        "{vendor} {version}",
        vec![("vendor", vendor), ("version", version), ("date", date), ("type", ty.to_string())],
    )]
}

pub fn processes() -> Vec<Entry> {
    let n = fs::read_dir("/proc")
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()))
                .count()
        })
        .unwrap_or(0);
    vec![entry("Processes", "{count}", vec![("count", n.to_string())])]
}

/// Dispatch a built-in module by name. Returns None when the name is not a built-in.
pub fn run(kind: &str, paths: Option<Vec<String>>) -> Option<Vec<Entry>> {
    Some(match kind {
        "os" => os(),
        "host" => host(),
        "kernel" => kernel(),
        "uptime" => uptime(),
        "packages" => packages(),
        "shell" => shell(),
        "display" => display(),
        "de" => de(),
        "wm" => wm(),
        "theme" => theme(),
        "icons" => icons(),
        "font" => font(),
        "cursor" => cursor(),
        "terminal" => terminal(),
        "terminalfont" => terminal_font(),
        "cpu" => cpu(),
        "gpu" => gpu(),
        "memory" => memory(),
        "swap" => swap(),
        "disk" => disk(paths),
        "localip" => localip(),
        "battery" => battery(),
        "locale" => locale(),
        "board" => board(),
        "bios" => bios(),
        "processes" => processes(),
        _ => return None,
    })
}

pub const BUILTINS: &[&str] = &[
    "os", "host", "kernel", "uptime", "packages", "shell", "display", "de", "wm", "theme", "icons", "font", "cursor",
    "terminal", "terminalfont", "cpu", "gpu", "memory", "swap", "disk", "localip", "battery", "locale", "board", "bios", "processes",
];

pub fn is_builtin(kind: &str) -> bool {
    BUILTINS.contains(&kind)
}

/// Modules worth a thread of their own. Starting a thread costs about as much as most built-in
/// modules take to run (the slow ones are cached), so only `command` modules, which start
/// processes, are overlapped.
pub fn is_slow(kind: &str) -> bool {
    matches!(kind, "command")
}

/// Run a shell command for the `command` module type.
pub fn run_command(cmd: &str, shell: &str, all_lines: bool) -> Vec<Entry> {
    let out = Command::new(shell)
        .arg("-c")
        .arg(cmd)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(out) = out else { return vec![] };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines: Vec<&str> = text.lines().collect();
    if !all_lines {
        lines.truncate(1);
    }
    lines
        .into_iter()
        .map(|l| entry("", "{output}", vec![("output", l.to_string())]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edid() -> Vec<u8> {
        let mut e = vec![0u8; 128];
        e[8] = 0x0D; // vendor "CMN"
        e[9] = 0xAE;
        e[10] = 0x21; // product 0x1521
        e[11] = 0x15;
        e[21] = 34; // 34 x 19 cm
        e[22] = 19;
        e
    }

    #[test]
    fn edid_vendor_and_model_name() {
        assert_eq!(edid_name(&edid()).as_deref(), Some("CMN1521"));
    }

    #[test]
    fn edid_model_descriptor_wins() {
        let mut e = edid();
        e[0x36 + 3] = 0xfc;
        e[0x36 + 5..0x36 + 10].copy_from_slice(b"DELL\n");
        assert_eq!(edid_name(&e).as_deref(), Some("DELL"));
    }

    #[test]
    fn edid_size_gives_15_inch() {
        let (w, h) = edid_size_mm(&edid()).unwrap();
        assert_eq!(((((w * w + h * h) as f64).sqrt() / 25.4) + 0.5) as u32, 15);
    }

    #[test]
    fn durations() {
        assert_eq!(duration_text(4620), "1 hour, 17 mins");
        assert_eq!(duration_text(60), "1 min");
    }
}
