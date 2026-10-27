use crate::config::*;
use crate::logo::{self, LogoDef};
use crate::modules::{self, Entry};
use crate::util::*;
use mlua::Value;
use std::time::Instant;

pub struct RunOpts {
    pub no_logo: bool,
    pub bench: bool,
}

// ---------------------------------------------------------------- formatting

/// Replace `{name}` placeholders.
///
/// - `{field}`      a field of the current module (`{user}` and `{host}` always work)
/// - `{#bold red}`  switch color / style, `{#}` resets
/// - `{env:HOME}`   an environment variable
/// - `{{` `}}`      literal braces
pub fn apply_format(fmt: &str, fields: &[(String, String)]) -> String {
    let mut out = String::with_capacity(fmt.len() + 16);
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut name = String::new();
                let mut closed = false;
                for d in chars.by_ref() {
                    if d == '}' {
                        closed = true;
                        break;
                    }
                    name.push(d);
                }
                if !closed {
                    out.push('{');
                    out.push_str(&name);
                } else if let Some(spec) = name.strip_prefix('#') {
                    if spec.is_empty() {
                        out.push_str(&reset());
                    } else if let Some(p) = color_params(spec) {
                        out.push_str(&sgr(&p));
                    }
                } else if let Some(var) = name.strip_prefix("env:") {
                    out.push_str(&std::env::var(var).unwrap_or_default());
                } else if let Some((_, v)) = fields.iter().find(|(k, _)| *k == name) {
                    out.push_str(v);
                } else if name == "user" {
                    out.push_str(&modules::username());
                } else if name == "host" {
                    out.push_str(&modules::hostname());
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn truthy(v: &Value) -> bool {
    !matches!(v, Value::Nil | Value::Boolean(false))
}

/// Turn whatever a Lua function returned into output lines.
fn value_to_lines(v: Value) -> mlua::Result<Vec<String>> {
    Ok(match v {
        Value::Nil | Value::Boolean(false) => vec![],
        Value::String(s) => s.to_str()?.split('\n').map(|l| l.to_string()).collect(),
        Value::Integer(i) => vec![i.to_string()],
        Value::Number(n) => vec![n.to_string()],
        Value::Table(t) => {
            let mut lines = vec![];
            for item in t.sequence_values::<Value>() {
                lines.extend(value_to_lines(item?)?);
            }
            lines
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "expected a string or a list of strings, got {}",
                other.type_name()
            )));
        }
    })
}

// ---------------------------------------------------------------- logo

struct LogoPlan {
    art: Option<String>,
    colors: Vec<String>, // raw SGR params for $1..$9
    key_default: String,
    title_default: String,
}

fn plan_logo(cfg: &Config, no_logo: bool) -> LogoPlan {
    let lc = &cfg.logo;
    let src = lc.source.as_str();

    let mut art: Option<String> = None;
    let def: &'static LogoDef = if src == "auto" || src.is_empty() {
        let d = logo::detect(lc.small);
        art = Some(d.art.to_string());
        d
    } else if src.eq_ignore_ascii_case("random") {
        let d = logo::random(lc.small);
        art = Some(d.art.to_string());
        d
    } else if src.eq_ignore_ascii_case("none") {
        logo::detect(lc.small)
    } else if let Some(d) = logo::find(src, lc.small).or_else(|| logo::find_any(src)) {
        art = Some(d.art.to_string());
        d
    } else if let Ok(text) = std::fs::read_to_string(src) {
        art = Some(text.trim_end_matches('\n').to_string());
        logo::detect(lc.small)
    } else {
        eprintln!("nyfetch: logo `{}` is not a built-in logo or a readable file", src);
        logo::detect(lc.small)
    };

    // Inline art from the config wins over `source`.
    if let Some(t) = &lc.text {
        art = Some(t.trim_end_matches('\n').to_string());
    } else if let Some(l) = &lc.lines {
        art = Some(l.join("\n"));
    }
    if no_logo {
        art = None;
    }

    let n = def.colors.len().max(lc.colors.len());
    let colors: Vec<String> = (0..n)
        .map(|i| match lc.colors.get(i) {
            Some(c) if !c.is_empty() => c.clone(),
            _ => def.colors.get(i).map(|s| s.to_string()).unwrap_or_default(),
        })
        .collect();

    // Same defaults as Fastfetch: keys use the logo's key color, else its second color.
    let key_default = def
        .key_color
        .or_else(|| def.colors.get(1).copied())
        .or_else(|| def.colors.first().copied())
        .unwrap_or("")
        .to_string();
    let title_default = def.title_color.or_else(|| def.colors.first().copied()).unwrap_or("").to_string();
    LogoPlan { art, colors, key_default, title_default }
}

// ---------------------------------------------------------------- jobs

enum Job {
    Skip,
    Builtin(String, Option<Vec<String>>),
    Command(String, String, bool),
}

fn job_kind(j: &Job) -> &str {
    match j {
        Job::Builtin(k, _) => k,
        Job::Command(..) => "command",
        Job::Skip => "",
    }
}

fn run_job(j: &Job) -> Vec<Entry> {
    match j {
        Job::Skip => vec![],
        Job::Builtin(k, paths) => modules::run(k, paths.clone()).unwrap_or_default(),
        Job::Command(cmd, shell, all) => modules::run_command(cmd, shell, *all),
    }
}

fn plan_job(spec: &ModuleSpec) -> mlua::Result<Job> {
    if modules::is_builtin(&spec.kind) {
        let paths = if spec.kind == "disk" { spec.opts.get::<Option<Vec<String>>>("paths")? } else { None };
        return Ok(Job::Builtin(spec.kind.clone(), paths));
    }
    if spec.kind == "command" {
        let cmd = match spec.opts.get::<Option<String>>("cmd")? {
            Some(c) => c,
            None => spec.opts.get::<Option<String>>("command")?.unwrap_or_default(),
        };
        let shell = spec.opts.get::<Option<String>>("shell")?.unwrap_or_else(|| "sh".into());
        let all = spec.opts.get::<Option<bool>>("lines")?.unwrap_or(false);
        return Ok(Job::Command(cmd, shell, all));
    }
    Ok(Job::Skip)
}

// ---------------------------------------------------------------- run

struct Style<'a> {
    cfg: &'a Config,
    key_params: String,
    title_params: String,
}

impl Style<'_> {
    fn key(&self, spec: Option<&ModuleSpec>, key: &str) -> String {
        if key.is_empty() {
            return String::new();
        }
        let padded = if self.cfg.key_width > 0 {
            let w = display_width(key);
            format!("{}{}", key, " ".repeat(self.cfg.key_width.saturating_sub(w)))
        } else {
            key.to_string()
        };
        let params = match spec.and_then(|s| s.key_color.as_deref()) {
            Some(c) => color_params(c).unwrap_or_default(),
            None => self.key_params.clone(),
        };
        let bold = if self.cfg.bold { sgr("1") } else { String::new() };
        if bold.is_empty() && params.is_empty() {
            padded
        } else {
            format!("{}{}{}{}{}", reset(), bold, sgr(&params), padded, reset())
        }
    }

    fn separator(&self, spec: Option<&ModuleSpec>) -> String {
        let text = spec.and_then(|s| s.separator.as_deref()).unwrap_or(&self.cfg.separator);
        match self.cfg.sep_color.as_deref().and_then(color_params) {
            Some(p) => format!("{}{}{}", sgr(&p), text, reset()),
            None => text.to_string(),
        }
    }

    fn value(&self, spec: Option<&ModuleSpec>, value: &str) -> String {
        let params = spec
            .and_then(|s| s.value_color.as_deref())
            .or(self.cfg.text_color.as_deref())
            .and_then(color_params);
        match params {
            Some(p) => format!("{}{}{}", sgr(&p), value, reset()),
            None => format!("{}{}", reset(), value),
        }
    }

    fn line(&self, spec: Option<&ModuleSpec>, key: &str, value: &str) -> String {
        if key.is_empty() {
            self.value(spec, value)
        } else {
            format!("{}{}{}", self.key(spec, key), self.separator(spec), self.value(spec, value))
        }
    }
}

fn title_text(cfg: &Config, spec: Option<&ModuleSpec>) -> mlua::Result<String> {
    let fields: [(String, String); 0] = [];
    Ok(match spec.and_then(|s| s.format.as_ref()) {
        Some(Format::Str(f)) => apply_format(f, &fields),
        Some(Format::Func(f)) => value_to_lines(f.call::<Value>(())?)?.join(" "),
        None => {
            let _ = cfg;
            format!("{}@{}", modules::username(), modules::hostname())
        }
    })
}

fn eval_when(spec: &ModuleSpec) -> mlua::Result<bool> {
    Ok(match &spec.when {
        When::Always => true,
        When::Never => false,
        When::Func(f) => truthy(&f.call::<Value>(())?),
    })
}

pub fn run(cfg: &Config, opts: &RunOpts) -> mlua::Result<String> {
    let plan = plan_logo(cfg, opts.no_logo);
    let style = Style {
        cfg,
        key_params: match cfg.key_color.as_deref() {
            Some(c) if c != "auto" => color_params(c).unwrap_or_default(),
            _ => plan.key_default.clone(),
        },
        title_params: match cfg.title_color.as_deref() {
            Some(c) if c != "auto" => color_params(c).unwrap_or_default(),
            _ => plan.title_default.clone(),
        },
    };

    // 1. Decide what to do for every module (evaluates `when` conditions, which may call Lua).
    let mut active = vec![true; cfg.items.len()];
    let mut jobs: Vec<Job> = Vec::with_capacity(cfg.items.len());
    for (i, item) in cfg.items.iter().enumerate() {
        match item {
            Item::Spec(spec) => {
                active[i] = eval_when(spec)?;
                jobs.push(if active[i] { plan_job(spec)? } else { Job::Skip });
            }
            Item::Func(_) => jobs.push(Job::Skip),
        }
    }

    // 2. Gather data. Slow modules run on threads so their waiting overlaps.
    let mut results: Vec<Vec<Entry>> = (0..jobs.len()).map(|_| vec![]).collect();
    if opts.bench {
        let total = Instant::now();
        for (i, job) in jobs.iter().enumerate() {
            if matches!(job, Job::Skip) {
                continue;
            }
            let t = Instant::now();
            results[i] = run_job(job);
            eprintln!("{:>8.0} us  {}", t.elapsed().as_secs_f64() * 1e6, job_kind(job));
        }
        eprintln!("{:>8.0} us  (all modules, sequential)", total.elapsed().as_secs_f64() * 1e6);
    } else if cfg.threads {
        std::thread::scope(|s| {
            let mut handles = vec![];
            for (i, job) in jobs.iter().enumerate() {
                if !matches!(job, Job::Skip) && modules::is_slow(job_kind(job)) {
                    handles.push((i, s.spawn(move || run_job(job))));
                }
            }
            for (i, job) in jobs.iter().enumerate() {
                if !matches!(job, Job::Skip) && !modules::is_slow(job_kind(job)) {
                    results[i] = run_job(job);
                }
            }
            for (i, h) in handles {
                results[i] = h.join().unwrap_or_default();
            }
        });
    } else {
        for (i, job) in jobs.iter().enumerate() {
            results[i] = run_job(job);
        }
    }

    // 3. Width of the title, used by `separator`.
    let title_spec = cfg.items.iter().enumerate().find_map(|(i, it)| match it {
        Item::Spec(s) if s.kind == "title" && active[i] => Some(s),
        _ => None,
    });
    let title_len = display_width(&strip_ansi(&title_text(cfg, title_spec)?));

    // 4. Build the information lines in order.
    let mut info: Vec<String> = vec![];
    for (i, item) in cfg.items.iter().enumerate() {
        if !active[i] {
            continue;
        }
        let spec = match item {
            Item::Func(f) => {
                info.extend(value_to_lines(f.call::<Value>(())?)?);
                continue;
            }
            Item::Spec(s) => s,
        };
        let sp = Some(spec);
        match spec.kind.as_str() {
            "title" => {
                let default_title = spec.format.is_none();
                let text = title_text(cfg, Some(spec))?;
                let params = match spec.opts.get::<Option<String>>("color")? {
                    Some(c) => color_params(&c).unwrap_or_default(),
                    None => style.title_params.clone(),
                };
                let bold = if cfg.bold { "1" } else { "" };
                let joined = [bold, params.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(";");
                info.push(if default_title {
                    // user@host with the user and host colored separately, like Fastfetch.
                    let (u, h) = (modules::username(), modules::hostname());
                    let b = if cfg.bold { sgr("1") } else { String::new() };
                    format!("{r}{b}{c}{u}{r}@{b}{c}{h}{r}", r = reset(), b = b, c = sgr(&params), u = u, h = h)
                } else if joined.is_empty() {
                    text
                } else {
                    format!("{}{}{}", sgr(&joined), text, reset())
                });
            }
            "separator" => {
                let ch = spec.opts.get::<Option<String>>("string")?.unwrap_or_else(|| "-".into());
                let len = spec.opts.get::<Option<usize>>("length")?.unwrap_or(title_len);
                let unit = display_width(&ch).max(1);
                let line = ch.repeat(len / unit);
                info.push(match spec.opts.get::<Option<String>>("color")?.as_deref().and_then(color_params) {
                    Some(p) => format!("{}{}{}", sgr(&p), line, reset()),
                    None => line,
                });
            }
            "break" => {
                for _ in 0..spec.opts.get::<Option<usize>>("count")?.unwrap_or(1) {
                    info.push(String::new());
                }
            }
            "colors" => {
                let symbol = spec.opts.get::<Option<String>>("symbol")?.unwrap_or_else(|| "background".into());
                let width = spec.opts.get::<Option<usize>>("width")?.unwrap_or(3);
                let left = " ".repeat(spec.opts.get::<Option<usize>>("paddingLeft")?.unwrap_or(0));
                let rows = spec.opts.get::<Option<String>>("rows")?.unwrap_or_else(|| "both".into());
                let background = symbol == "background" || symbol.trim().is_empty();
                let glyph = match symbol.as_str() {
                    "block" => "█".to_string(),
                    "background" => " ".to_string(),
                    s if s.trim().is_empty() => " ".to_string(),
                    s => s.to_string(),
                };
                let make = |base: u32| {
                    let mut row = left.clone();
                    if background {
                        if base == 30 {
                            row.push_str(&reset());
                        } else {
                            let t = std::env::var("TERM").unwrap_or_default();
                            if !t.is_empty() && !t.starts_with("xterm") {
                                row.push_str(&sgr("5"));
                            }
                        }
                    }
                    for i in 0..8 {
                        let code = if background { 40 + (base - 30) + i } else { base + i };
                        row.push_str(&sgr(&code.to_string()));
                        row.push_str(&glyph.repeat(width));
                    }
                    if crate::util::COLOR.load(std::sync::atomic::Ordering::Relaxed) {
                        row.push_str("\x1b[m");
                    }
                    row
                };
                if rows == "both" || rows == "normal" {
                    info.push(make(30));
                }
                if rows == "both" || rows == "bright" {
                    info.push(make(90));
                }
            }
            "text" | "custom" => {
                let lines = if let Some(t) = spec.opts.get::<Option<String>>("text")? {
                    apply_format(&t, &[]).split('\n').map(|l| l.to_string()).collect()
                } else {
                    match &spec.format {
                        Some(Format::Str(f)) => apply_format(f, &[]).split('\n').map(|l| l.to_string()).collect(),
                        Some(Format::Func(f)) => value_to_lines(f.call::<Value>(())?)?,
                        None => match spec.opts.get::<Option<String>>("value")? {
                            Some(v) => vec![apply_format(&v, &[])],
                            None => vec![],
                        },
                    }
                };
                let key = spec.key.as_deref().map(|k| apply_format(k, &[])).unwrap_or_default();
                for (n, l) in lines.iter().enumerate() {
                    if n == 0 {
                        info.push(style.line(sp, &key, l));
                    } else if key.is_empty() {
                        info.push(style.value(sp, l));
                    } else {
                        // Continuation lines are aligned under the first value.
                        let pad = display_width(&key).max(cfg.key_width) + display_width(&style.separator(sp));
                        info.push(format!("{}{}", " ".repeat(pad), style.value(sp, l)));
                    }
                }
            }
            kind => {
                if matches!(jobs[i], Job::Skip) {
                    let best = modules::BUILTINS
                        .iter()
                        .map(|b| (edit_distance(kind, b), *b))
                        .min()
                        .filter(|(d, _)| *d <= 2)
                        .map(|(_, b)| format!(" Did you mean \"{}\"?", b))
                        .unwrap_or_default();
                    eprintln!("nyfetch: there is no module called \"{}\".{} (see the module list in your config file)", kind, best);
                    continue;
                }
                for e in &results[i] {
                    let value = match &spec.format {
                        Some(Format::Str(f)) => apply_format(f, &e.fields),
                        Some(Format::Func(f)) => {
                            let t = cfg.lua.create_table()?;
                            for (k, v) in &e.fields {
                                t.set(k.as_str(), v.as_str())?;
                            }
                            let lines = value_to_lines(f.call::<Value>(t)?)?;
                            if lines.is_empty() {
                                continue;
                            }
                            lines.join(" ")
                        }
                        None => apply_format(&e.fmt, &e.fields),
                    };
                    if value.is_empty() {
                        continue;
                    }
                    let key_tpl = spec.key.as_deref().unwrap_or(&e.key);
                    let key = apply_format(key_tpl, &e.fields);
                    info.push(style.line(sp, &key, &value));
                }
            }
        }
    }

    // 5. Lay out logo and information.
    let lc = &cfg.logo;
    let built = plan.art.as_ref().map(|a| logo::build(a, &plan.colors, lc.bold.unwrap_or(cfg.bold)));

    if let Some(hook) = &cfg.render {
        let logo_t = cfg.lua.create_table()?;
        if let Some(b) = &built {
            for (n, l) in b.lines.iter().enumerate() {
                logo_t.set(n + 1, l.as_str())?;
            }
        }
        let info_t = cfg.lua.create_table()?;
        for (n, l) in info.iter().enumerate() {
            info_t.set(n + 1, l.as_str())?;
        }
        let ctx = cfg.lua.create_table()?;
        ctx.set("logo_width", built.as_ref().map(|b| b.width.max(lc.width)).unwrap_or(0))?;
        ctx.set("padding_left", lc.pad_left)?;
        ctx.set("padding_right", lc.pad_right)?;
        let mut s = value_to_lines(hook.call::<Value>((logo_t, info_t, ctx))?)?.join("\n");
        s.push('\n');
        return Ok(s);
    }

    let mut out = String::with_capacity(4096);
    let Some(b) = built else {
        for l in &info {
            out.push_str(l);
            out.push('\n');
        }
        return Ok(out);
    };
    let width = b.width.max(lc.width);

    match lc.position {
        Position::Top => {
            for _ in 0..lc.pad_top {
                out.push('\n');
            }
            for l in &b.lines {
                out.push_str(&" ".repeat(lc.pad_left));
                out.push_str(l);
                out.push('\n');
            }
            for _ in 0..lc.pad_bottom {
                out.push('\n');
            }
            for l in &info {
                out.push_str(l);
                out.push('\n');
            }
        }
        Position::Left | Position::Right => {
            let logo_h = b.lines.len();
            let offset = lc.pad_top
                + match lc.valign {
                    VAlign::Top => 0,
                    VAlign::Center => info.len().saturating_sub(logo_h + lc.pad_top) / 2,
                    VAlign::Bottom => info.len().saturating_sub(logo_h + lc.pad_top),
                };
            let rows = info.len().max(offset + logo_h);
            let info_w = info.iter().map(|l| display_width(l)).max().unwrap_or(0);
            for i in 0..rows {
                let logo_idx = i.checked_sub(offset).filter(|&n| n < logo_h);
                let (l, w) = match logo_idx {
                    Some(n) => (b.lines[n].as_str(), b.widths[n]),
                    None => ("", 0),
                };
                if lc.position == Position::Left {
                    out.push_str(&" ".repeat(lc.pad_left));
                    out.push_str(l);
                    out.push_str(&" ".repeat(width - w + lc.pad_right));
                    if let Some(line) = info.get(i) {
                        out.push_str(line);
                    }
                } else {
                    let line = info.get(i).map(|s| s.as_str()).unwrap_or("");
                    out.push_str(line);
                    if !l.is_empty() {
                        out.push_str(&" ".repeat(info_w - display_width(line) + lc.pad_right));
                        out.push_str(l);
                    }
                }
                out.push('\n');
            }
        }
    }
    Ok(out)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}
