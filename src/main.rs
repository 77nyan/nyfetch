mod config;
mod logo;
mod logos_data;
mod modules;
mod render;
mod util;

use std::io::{IsTerminal, Write};
use std::sync::atomic::Ordering;

const HELP: &str = "\
NyFetch - fast system information, configured in Lua

USAGE:
    nyfetch [OPTIONS]

OPTIONS:
    -c, --config <FILE>     Use this Lua config instead of ~/.config/nyfetch/config.lua
        --gen-config        Print the default config to stdout
    -l, --logo <NAME|FILE>  Override the logo (built-in name, text file, `random` or `none`)
        --small             Prefer the small variant of the logo
        --no-logo           Do not print a logo
        --list-logos        List all built-in logo names
        --no-cache          Do not use the on-disk cache
        --pipe              Force plain output without colors
        --bench             Print how long each module takes
    -h, --help              Show this help
    -V, --version           Show the version
";

fn main() {
    let mut config_path: Option<String> = None;
    let mut logo_override: Option<String> = None;
    let mut small = false;
    let mut no_logo = false;
    let mut pipe = false;
    let mut bench = false;
    let mut no_cache = false;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-c" | "--config" => match args.next() {
                Some(p) => config_path = Some(p),
                None => return fail("--config needs a file path"),
            },
            "-l" | "--logo" => match args.next() {
                Some(l) => logo_override = Some(l),
                None => return fail("--logo needs a name or path"),
            },
            "--small" => small = true,
            "--no-logo" => no_logo = true,
            "--no-cache" => no_cache = true,
            "--pipe" => pipe = true,
            "--bench" => bench = true,
            "--gen-config" => {
                print!("{}", config::DEFAULT_CONFIG);
                return;
            }
            "--list-logos" => {
                let mut out = String::new();
                for n in logo::list_names(false) {
                    out.push_str(n);
                    out.push('\n');
                }
                let _ = std::io::stdout().write_all(out.as_bytes());
                return;
            }
            "-h" | "--help" => {
                print!("{}", HELP);
                return;
            }
            "-V" | "--version" => {
                println!("nyfetch {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            other => return fail(&format!("unknown option `{}` (try --help)", other)),
        }
    }

    let colors = !pipe && std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    util::COLOR.store(colors, Ordering::Relaxed);
    if no_cache {
        util::CACHE.store(false, Ordering::Relaxed);
    }

    let mut cfg = match config::load(config_path.as_deref()) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    if let Some(l) = logo_override {
        cfg.logo.source = l;
        cfg.logo.text = None;
        cfg.logo.lines = None;
    }
    if small {
        cfg.logo.small = true;
    }

    match render::run(&cfg, &render::RunOpts { no_logo, bench }) {
        Ok(out) => {
            let mut stdout = std::io::stdout().lock();
            let _ = stdout.write_all(out.as_bytes());
            if bench {
                if let Some(status) = util::read("/proc/self/status") {
                    for line in status.lines().filter(|l| l.starts_with("VmHWM") || l.starts_with("RssAnon") || l.starts_with("RssFile")) {
                        eprintln!("{}", line.split_whitespace().collect::<Vec<_>>().join(" "));
                    }
                }
            }
        }
        Err(e) => fail(&format!("error while running modules:\n{}", e)),
    }
}

fn fail(msg: &str) {
    eprintln!("nyfetch: {}", msg);
    std::process::exit(1);
}
