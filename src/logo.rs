use crate::util::*;

pub const FLAG_SMALL: u8 = 1;

/// A built-in logo (imported from Fastfetch, see tools/import_logos.py).
pub struct LogoDef {
    pub names: &'static [&'static str],
    /// Default palette as raw SGR parameters; `$1`..`$9` in the art refer to these.
    pub colors: &'static [&'static str],
    pub key_color: Option<&'static str>,
    pub title_color: Option<&'static str>,
    pub flags: u8,
    pub art: &'static str,
}

pub use crate::logos_data::{LOGOS, UNKNOWN};

fn eq_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase()
}

fn has_name(logo: &LogoDef, name: &str, small: bool) -> bool {
    logo.names.iter().any(|n| {
        if small {
            let lower = n.to_ascii_lowercase();
            if lower.ends_with("_small") || lower.ends_with("-small") {
                let stem = &n[..n.len() - 6];
                if eq_name(stem, name) {
                    return true;
                }
            }
        }
        eq_name(n, name)
    })
}

/// Look up a built-in logo by name or alias. `small` selects the compact variants.
pub fn find(name: &str, small: bool) -> Option<&'static LogoDef> {
    if !name.chars().next().map_or(false, |c| c.is_alphabetic()) {
        return None;
    }
    LOGOS.iter().find(|l| {
        let kind_ok = if small { l.flags == FLAG_SMALL } else { l.flags == 0 };
        kind_ok && has_name(l, name, small)
    })
}

/// Same lookup, but any variant (used by `--logo some_name_small`).
pub fn find_any(name: &str) -> Option<&'static LogoDef> {
    LOGOS.iter().find(|l| l.names.iter().any(|n| eq_name(n, name)))
}

/// Pick the logo for this system the way Fastfetch does: os-release ID, then NAME, then each
/// ID_LIKE entry, then the kernel name.
pub fn detect(small: bool) -> &'static LogoDef {
    let rel = crate::modules::os_release();
    let get = |k: &str| rel.get(k).map(|s| s.as_str()).unwrap_or("");
    if let Some(l) = find(get("ID"), small) {
        return l;
    }
    if let Some(l) = find(get("NAME"), small) {
        return l;
    }
    for like in get("ID_LIKE").split_whitespace() {
        if let Some(l) = find(like, small) {
            return l;
        }
    }
    if let Some(l) = find(&crate::modules::uname().sysname, small) {
        return l;
    }
    &UNKNOWN
}

pub fn random(small: bool) -> &'static LogoDef {
    let pool: Vec<&LogoDef> = LOGOS.iter().filter(|l| if small { l.flags == FLAG_SMALL } else { l.flags == 0 }).collect();
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize ^ d.as_secs() as usize)
        .unwrap_or(0);
    pool.get(seed % pool.len().max(1)).copied().unwrap_or(&UNKNOWN)
}

pub fn list_names(small: bool) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = LOGOS
        .iter()
        .filter(|l| if small { l.flags == FLAG_SMALL } else { true })
        .map(|l| l.names[0])
        .collect();
    v.sort_by_key(|s| s.to_lowercase());
    v
}

pub struct Built {
    pub lines: Vec<String>,
    pub widths: Vec<usize>,
    pub width: usize,
}

/// Expand logo art into ready-to-print lines.
///
/// Understands the same syntax as Fastfetch: `$1`..`$9` switch to palette color N (and the color
/// carries onto following lines), `$$` is a literal dollar, tabs are 4 spaces and raw
/// `ESC[..m` sequences pass through. `colors` are raw SGR parameters.
pub fn build(art: &str, colors: &[String], bold: bool) -> Built {
    let on = color_on();
    let esc = |i: usize| format!("\x1b[{}m", colors.get(i).map(|s| s.as_str()).unwrap_or(""));
    let mut carry = if on { esc(0) } else { String::new() };
    let mut lines = vec![];
    let mut widths = vec![];
    let mut max = 0;

    for raw in art.lines() {
        let mut line = String::with_capacity(raw.len() + 16);
        let mut w = 0;
        if on && bold {
            line.push_str("\x1b[1m");
        }
        line.push_str(&carry);

        let mut it = raw.chars().peekable();
        while let Some(c) = it.next() {
            match c {
                '\t' => {
                    line.push_str("    ");
                    w += 4;
                }
                '\x1b' if it.peek() == Some(&'[') => {
                    let mut seq = String::from("\x1b[");
                    it.next();
                    for d in it.by_ref() {
                        seq.push(d);
                        if ('@'..='~').contains(&d) {
                            break;
                        }
                    }
                    if on {
                        line.push_str(&seq);
                        if seq.ends_with('m') {
                            carry = seq;
                        }
                    }
                }
                '$' => match it.peek().copied() {
                    None | Some('$') => {
                        it.next();
                        line.push('$');
                        w += 1;
                    }
                    Some(d) if !on => {
                        // Color tokens are dropped when output is plain.
                        let _ = d;
                        it.next();
                    }
                    Some(d) if ('1'..='9').contains(&d) => {
                        it.next();
                        carry = esc(d as usize - '1' as usize);
                        line.push_str(&carry);
                    }
                    Some(_) => {
                        line.push('$');
                        w += 1;
                    }
                },
                _ => {
                    line.push(c);
                    w += char_width(c);
                }
            }
        }
        if on {
            line.push_str("\x1b[m");
        }
        max = max.max(w);
        lines.push(line);
        widths.push(w);
    }
    Built { lines, widths, width: max }
}
