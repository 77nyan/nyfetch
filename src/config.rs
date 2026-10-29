use crate::modules;
use crate::render::apply_format;
use crate::util::*;
use mlua::{Function, Lua, LuaOptions, MultiValue, StdLib, Table, Value};
use std::process::Command;
use std::sync::atomic::Ordering::Relaxed;

pub const DEFAULT_CONFIG: &str = include_str!("../default_config.lua");

pub enum Format {
    Str(String),
    Func(Function),
}

pub enum When {
    Always,
    Never,
    Func(Function),
}

pub struct ModuleSpec {
    pub kind: String,
    pub key: Option<String>,
    pub format: Option<Format>,
    pub key_color: Option<String>,
    pub value_color: Option<String>,
    pub separator: Option<String>,
    pub when: When,
    pub opts: Table,
}

pub enum Item {
    Spec(ModuleSpec),
    /// A bare Lua function in the modules list: returns a string, a list of strings, or nil.
    Func(Function),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Position {
    Left,
    Right,
    Top,
}

#[derive(Clone, Copy, PartialEq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

pub struct LogoCfg {
    pub source: String,
    pub small: bool,
    pub text: Option<String>,
    pub lines: Option<Vec<String>>,
    pub colors: Vec<String>,
    pub position: Position,
    pub valign: VAlign,
    pub pad_top: usize,
    pub pad_left: usize,
    pub pad_right: usize,
    pub pad_bottom: usize,
    pub width: usize,
    pub bold: Option<bool>,
}

pub struct Config {
    pub lua: Lua,
    pub logo: LogoCfg,
    pub threads: bool,
    pub separator: String,
    pub key_width: usize,
    pub bold: bool,
    pub key_color: Option<String>,
    pub title_color: Option<String>,
    pub sep_color: Option<String>,
    pub text_color: Option<String>,
    pub render: Option<Function>,
    pub items: Vec<Item>,
}

fn terminal_size() -> (usize, usize) {
    for fd in [1, 2, 0] {
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } == 0 && ws.ws_col > 0 {
            return (ws.ws_col as usize, ws.ws_row as usize);
        }
    }
    let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    (env("COLUMNS", 80), env("LINES", 24))
}

fn install_helpers(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();

    // exec("cmd") -> trimmed stdout, or nil when the command fails
    g.set(
        "exec",
        lua.create_function(|_, cmd: String| {
            let out = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output();
            Ok(match out {
                Ok(o) if o.status.success() => Some(String::from_utf8_lossy(&o.stdout).trim().to_string()),
                _ => None,
            })
        })?,
    )?;
    g.set("env", lua.create_function(|_, name: String| Ok(std::env::var(name).ok()))?)?;
    g.set("read_file", lua.create_function(|_, path: String| Ok(read(&path)))?)?;
    g.set(
        "file_exists",
        lua.create_function(|_, path: String| Ok(std::path::Path::new(&path).exists()))?,
    )?;

    // ansi("bold red") -> escape sequence; ansi() -> reset
    g.set(
        "ansi",
        lua.create_function(|_, spec: Option<String>| {
            Ok(match spec {
                None => reset(),
                Some(s) => color_params(&s).map(|p| sgr(&p)).unwrap_or_default(),
            })
        })?,
    )?;

    // bar(percent [, width [, filled [, empty]]])
    g.set(
        "bar",
        lua.create_function(|_, (pct, width, full, empty): (f64, Option<usize>, Option<String>, Option<String>)| {
            let w = width.unwrap_or(10);
            let filled = ((pct.clamp(0.0, 100.0) / 100.0) * w as f64).round() as usize;
            let full = full.unwrap_or_else(|| "█".into());
            let empty = empty.unwrap_or_else(|| "░".into());
            Ok(format!("{}{}", full.repeat(filled), empty.repeat(w - filled)))
        })?,
    )?;

    g.set("bytes", lua.create_function(|_, n: f64| Ok(fmt_bytes(n.max(0.0) as u64)))?)?;
    g.set("width", lua.create_function(|_, s: String| Ok(display_width(&s)))?)?;
    g.set("strip", lua.create_function(|_, s: String| Ok(strip_ansi(&s)))?)?;

    // gradient("some text", "#ff0000", "#0000ff") -> text colored per character
    g.set(
        "gradient",
        lua.create_function(|_, (text, from, to): (String, String, String)| {
            let (Some(a), Some(b)) = (rgb_of(&from), rgb_of(&to)) else {
                return Err(mlua::Error::runtime("gradient needs two colors like \"#ff0000\" or \"red\""));
            };
            let chars: Vec<char> = text.chars().collect();
            let n = chars.len().max(2) - 1;
            let mut out = String::new();
            for (i, c) in chars.iter().enumerate() {
                let t = i as f64 / n as f64;
                let mix = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
                out.push_str(&sgr(&format!("38;2;{};{};{}", mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))));
                out.push(*c);
            }
            out.push_str(&reset());
            Ok(out)
        })?,
    )?;

    // nyfetch.* : version, user, host, query(), terminal_size(), os_release()
    let ny = lua.create_table()?;
    ny.set("version", env!("CARGO_PKG_VERSION"))?;
    ny.set("user", modules::username())?;
    ny.set("host", modules::hostname())?;
    ny.set(
        "query",
        lua.create_function(|lua, (name, opts): (String, Option<Table>)| {
            let paths = match &opts {
                Some(t) => t.get::<Option<Vec<String>>>("paths")?,
                None => None,
            };
            let entries = modules::run(&name, paths)
                .ok_or_else(|| mlua::Error::runtime(format!("nyfetch.query: unknown module `{}`", name)))?;
            let out = lua.create_table()?;
            for (i, e) in entries.iter().enumerate() {
                let t = lua.create_table()?;
                for (k, v) in &e.fields {
                    t.set(k.as_str(), v.as_str())?;
                }
                t.set("key", apply_format(&e.key, &e.fields))?;
                t.set("value", apply_format(&e.fmt, &e.fields))?;
                out.set(i + 1, t)?;
            }
            Ok(out)
        })?,
    )?;
    ny.set(
        "terminal_size",
        lua.create_function(|_, ()| {
            let (c, r) = terminal_size();
            Ok(MultiValue::from_iter([Value::Integer(c as i64), Value::Integer(r as i64)]))
        })?,
    )?;
    ny.set(
        "os_release",
        lua.create_function(|lua, ()| {
            let t = lua.create_table()?;
            for (k, v) in modules::os_release() {
                t.set(k.as_str(), v.as_str())?;
            }
            Ok(t)
        })?,
    )?;
    g.set("nyfetch", ny)?;
    Ok(())
}

fn opt_string(t: &Table, key: &str) -> mlua::Result<Option<String>> {
    t.get::<Option<String>>(key)
}

fn opt_table(lua: &Lua, t: &Table, key: &str) -> mlua::Result<Table> {
    Ok(t.get::<Option<Table>>(key)?.unwrap_or(lua.create_table()?))
}

fn parse_item(lua: &Lua, v: Value) -> mlua::Result<Item> {
    match v {
        Value::String(s) => Ok(Item::Spec(ModuleSpec {
            kind: s.to_str()?.to_string(),
            key: None,
            format: None,
            key_color: None,
            value_color: None,
            separator: None,
            when: When::Always,
            opts: lua.create_table()?,
        })),
        Value::Function(f) => Ok(Item::Func(f)),
        Value::Table(t) => {
            let kind = opt_string(&t, "type")?.ok_or_else(|| mlua::Error::runtime("a module table needs a `type` field"))?;
            let format = match t.get::<Value>("format")? {
                Value::String(s) => Some(Format::Str(s.to_str()?.to_string())),
                Value::Function(f) => Some(Format::Func(f)),
                _ => None,
            };
            let hide = t.get::<Option<bool>>("hide")?.unwrap_or(false);
            let when = if hide {
                When::Never
            } else {
                match t.get::<Value>("when")? {
                    Value::Boolean(true) | Value::Nil => When::Always,
                    Value::Boolean(false) => When::Never,
                    Value::Function(f) => When::Func(f),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "`when` must be a boolean or a function, got {}",
                            other.type_name()
                        )));
                    }
                }
            };
            Ok(Item::Spec(ModuleSpec {
                kind,
                key: opt_string(&t, "key")?,
                format,
                key_color: opt_string(&t, "keyColor")?,
                value_color: opt_string(&t, "valueColor")?,
                separator: opt_string(&t, "separator")?,
                when,
                opts: t,
            }))
        }
        other => Err(mlua::Error::runtime(format!(
            "modules entries must be strings, tables or functions, got {}",
            other.type_name()
        ))),
    }
}

fn parse_logo(lua: &Lua, root: &Table) -> mlua::Result<LogoCfg> {
    // Shorthand for beginners: `logo = "arch"` instead of `logo = { source = "arch" }`.
    let logo = match root.get::<Value>("logo")? {
        Value::String(s) => {
            let t = lua.create_table()?;
            t.set("source", s)?;
            t
        }
        _ => opt_table(lua, root, "logo")?,
    };
    let padding = opt_table(lua, &logo, "padding")?;
    let colors = match logo.get::<Value>("color")? {
        Value::Table(t) => t.sequence_values::<String>().collect::<mlua::Result<Vec<_>>>()?,
        Value::String(s) => vec![s.to_str()?.to_string()],
        _ => vec![],
    };
    // Convert user color descriptions to SGR parameters now; empty entries keep the logo's own color.
    let colors = colors.iter().map(|c| color_params(c).unwrap_or_default()).collect();
    let lines = match logo.get::<Option<Table>>("lines")? {
        Some(t) => Some(t.sequence_values::<String>().collect::<mlua::Result<Vec<_>>>()?),
        None => None,
    };
    let position = match opt_string(&logo, "position")?.as_deref() {
        None | Some("left") => Position::Left,
        Some("right") => Position::Right,
        Some("top") => Position::Top,
        Some(o) => return Err(mlua::Error::runtime(format!("logo.position must be left, right or top, got `{}`", o))),
    };
    let valign = match opt_string(&logo, "valign")?.as_deref() {
        None | Some("top") => VAlign::Top,
        Some("center") | Some("middle") => VAlign::Center,
        Some("bottom") => VAlign::Bottom,
        Some(o) => return Err(mlua::Error::runtime(format!("logo.valign must be top, center or bottom, got `{}`", o))),
    };
    let num = |t: &Table, k: &str, d: usize| -> mlua::Result<usize> { Ok(t.get::<Option<usize>>(k)?.unwrap_or(d)) };
    Ok(LogoCfg {
        source: opt_string(&logo, "source")?.unwrap_or_else(|| "auto".into()),
        small: opt_string(&logo, "size")?.as_deref() == Some("small"),
        text: opt_string(&logo, "text")?,
        lines,
        colors,
        position,
        valign,
        pad_top: num(&padding, "top", 0)?,
        pad_left: num(&padding, "left", 0)?,
        pad_right: num(&padding, "right", 4)?,
        pad_bottom: num(&padding, "bottom", 1)?,
        width: num(&logo, "width", 0)?,
        bold: logo.get::<Option<bool>>("bold")?,
    })
}

const HINT: &str = "Hint: the line number above is where Lua noticed the problem (the real mistake is often a line or two earlier).\n\
Common causes: a missing comma at the end of a line, a missing closing quote or bracket, or text that is not inside quotes.\n\
To start over, run:  nyfetch --gen-config > ~/.config/nyfetch/config.lua";

pub fn load(path: Option<&str>) -> Result<Config, String> {
    // Only the libraries a config can reasonably want; leaves out debug, coroutine and friends.
    let libs = StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8 | StdLib::OS | StdLib::IO | StdLib::PACKAGE;
    let lua = Lua::new_with(libs, LuaOptions::default()).map_err(|e| e.to_string())?;
    // The process lives for a few milliseconds: never spend time collecting garbage.
    lua.gc_stop();
    install_helpers(&lua).map_err(|e| e.to_string())?;

    let (src, name, dir) = match path {
        Some(p) => {
            let dir = std::path::Path::new(p).parent().map(|d| d.display().to_string()).unwrap_or_else(|| ".".into());
            (std::fs::read_to_string(p).map_err(|e| format!("cannot read {}: {}", p, e))?, p.to_string(), dir)
        }
        None => {
            let home = std::env::var("HOME").unwrap_or_default();
            let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home));
            let dir = format!("{}/nyfetch", base);
            let p = format!("{}/config.lua", dir);
            match std::fs::read_to_string(&p) {
                Ok(s) => (s, p, dir),
                Err(_) => (DEFAULT_CONFIG.to_string(), "<built-in default config>".to_string(), dir),
            }
        }
    };

    // `require("mytheme")` finds ~/.config/nyfetch/mytheme.lua next to the config file.
    if let Ok(pkg) = lua.globals().get::<Table>("package") {
        let old = pkg.get::<String>("path").unwrap_or_default();
        let _ = pkg.set("path", format!("{d}/?.lua;{d}/?/init.lua;{old}", d = dir));
    }

    let default_table = || -> Result<Table, String> {
        lua.load(DEFAULT_CONFIG).set_name("=<built-in default config>").eval::<Table>().map_err(|e| e.to_string())
    };
    let evaluated: Value = lua
        .load(&src)
        .set_name(format!("={}", name))
        .eval()
        .map_err(|e| format!("config error in {}:\n{}\n\n{}", name, e, HINT))?;
    let root: Table = match evaluated {
        Value::Table(t) => t,
        // An empty config file (or one with only comments) simply means "use the defaults".
        Value::Nil => default_table()?,
        _ => {
            return Err(format!(
                "config error in {}:\nthe file must end with `return {{ ... }}` (a table).\n\n{}",
                name, HINT
            ));
        }
    };
    // A config without a `modules` list shows the default lines.
    if matches!(root.get::<Value>("modules"), Ok(Value::Nil)) {
        if let Ok(m) = default_table().and_then(|d| d.get::<Value>("modules").map_err(|e| e.to_string())) {
            let _ = root.set("modules", m);
        }
    }

    let build = || -> mlua::Result<Config> {
        let display = opt_table(&lua, &root, "display")?;
        let color = opt_table(&lua, &display, "color")?;
        let percent = opt_table(&lua, &display, "percent")?;
        let perf = opt_table(&lua, &root, "performance")?;

        // Global formatting switches
        if let Some(b) = display.get::<Option<bool>>("binary")? {
            BINARY.store(b, Relaxed);
        }
        if let Some(d) = display.get::<Option<u8>>("decimals")? {
            DECIMALS.store(d.min(9), Relaxed);
        }
        if let Some(b) = percent.get::<Option<bool>>("color")? {
            PCT_COLOR.store(b, Relaxed);
        }
        if let Some(n) = percent.get::<Option<u8>>("warn")? {
            PCT_WARN.store(n.min(100), Relaxed);
        }
        if let Some(n) = percent.get::<Option<u8>>("crit")? {
            PCT_CRIT.store(n.min(100), Relaxed);
        }
        if let Some(b) = perf.get::<Option<bool>>("cache")? {
            if !b {
                CACHE.store(false, Relaxed);
            }
        }
        if display.get::<Option<bool>>("pipe")? == Some(true) {
            COLOR.store(false, Relaxed);
        }

        let items = match root.get::<Option<Table>>("modules")? {
            Some(t) => t.sequence_values::<Value>().map(|v| parse_item(&lua, v?)).collect::<mlua::Result<Vec<_>>>()?,
            None => vec![],
        };

        Ok(Config {
            logo: parse_logo(&lua, &root)?,
            threads: perf.get::<Option<bool>>("threads")?.unwrap_or(true),
            separator: opt_string(&display, "separator")?.unwrap_or_else(|| ": ".into()),
            key_width: display.get::<Option<usize>>("keyWidth")?.unwrap_or(0),
            bold: display.get::<Option<bool>>("bold")?.unwrap_or(true),
            key_color: opt_string(&color, "keys")?,
            title_color: opt_string(&color, "title")?,
            sep_color: opt_string(&color, "separator")?,
            text_color: opt_string(&color, "text")?,
            render: root.get::<Option<Function>>("render")?,
            items,
            lua: lua.clone(),
        })
    };
    build().map_err(|e| format!("config error in {}:\n{}\n\n{}", name, e, HINT))
}
