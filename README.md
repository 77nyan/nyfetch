<div align="center">

# NyFetch

**A fast system info tool for Linux, written in Rust and configured in Lua.**<br>
Fastfetch's look and logos, with a config file anyone can edit.

[![Release](https://img.shields.io/github/v/release/77nyan/nyfetch?style=flat-square&color=a371f7)](https://github.com/77nyan/nyfetch/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-3fb950?style=flat-square)](LICENSE)
![Made with Rust](https://img.shields.io/badge/made%20with-Rust-dea584?style=flat-square&logo=rust&logoColor=white)
![Platform: Linux](https://img.shields.io/badge/platform-Linux-58a6ff?style=flat-square&logo=linux&logoColor=white)

⚡ **~3 ms** per run &nbsp;·&nbsp; 🪶 **~3 MB** of RAM &nbsp;·&nbsp; 🎨 **540+** logos &nbsp;·&nbsp; 🧩 **Lua** config

**[1.0 Install](#10-install)** &nbsp;·&nbsp; **[2.0 Usage](#20-usage)** &nbsp;·&nbsp; **[3.0 Configure](#30-configure)** &nbsp;·&nbsp; **[4.0 Roadmap](#40-roadmap)** &nbsp;·&nbsp; **[5.0 Credits](#50-credits)**

</div>

<br>

## 1.0 Install

```sh
sudo pacman -S --needed git
git clone https://github.com/77nyan/nyfetch.git
cd nyfetch
sudo ./install.sh
```

> [!IMPORTANT]
> Start the installer with `sudo ./install.sh` only. Running it with `sh install.sh` is refused on purpose.

Pick an option with <kbd>↑</kbd> <kbd>↓</kbd> (or <kbd>1</kbd>–<kbd>4</kbd>) and <kbd>Enter</kbd>:

| | Option | What it does |
|:-:|--------|--------------|
| 1 | **Install** | Builds NyFetch, installs it to `/usr/local/bin` and creates your config. |
| 2 | **Update** | Pulls the newest version from GitHub, rebuilds, keeps your config. |
| 3 | **Uninstall** | Removes NyFetch. Asks before touching your config. |
| 4 | **Quit** | Closes the installer. |

Then run `nyfetch` in any terminal.

<details>
<summary><b>Dependencies and other distributions</b></summary>

<br>

| Package | Used for |
|---------|----------|
| `rust` | builds NyFetch (the installer installs it on Arch) |
| `gcc` | compiles the bundled Lua interpreter (the installer installs it on Arch) |
| `git` | downloads the source and updates it |

Nothing is needed to *run* NyFetch, the binary is fully static. On other distributions install Rust ([rustup.rs](https://rustup.rs)) and `gcc` or `clang` yourself, then use the same four commands. To build by hand: `cargo build --release`.

If the installer says "permission denied", run `chmod +x install.sh` once.

</details>

<sub>[↑ back to top](#nyfetch)</sub>

## 2.0 Usage

| Command | Does |
|---------|------|
| `nyfetch` | show your system |
| `nyfetch --small` | use the small logo |
| `nyfetch -l arch` | use another logo (`random`, `none` or a text file work too) |
| `nyfetch --list-logos` | list all logos |
| `nyfetch --gen-config` | print the default config |
| `nyfetch -c file.lua` | use another config file |
| `nyfetch --bench` | time every line |

<sub>[↑ back to top](#nyfetch)</sub>

## 3.0 Configure

Edit `~/.config/nyfetch/config.lua`. It has simple settings and a list of lines at the top, and **a step-by-step tutorial at the bottom**, written for people who have never coded.

```lua
local logo = "arch"          -- pick a logo
local small_logo = true      -- smaller logo

local lines = {
  "title",
  "os",
  "kernel",
  -- "uptime",                                -- hidden: put -- in front
  { type = "memory", key = "RAM" },           -- renamed
  { type = "text", text = "Hello {user}!" },  -- your own text
  "colors",
}
```

<details>
<summary><b>All available lines</b></summary>

<br>

`title` `separator` `os` `host` `kernel` `uptime` `packages` `shell` `display` `de` `wm` `theme` `icons` `font` `cursor` `terminal` `terminalfont` `cpu` `gpu` `memory` `swap` `disk` `localip` `battery` `locale` `board` `bios` `processes` `break` `colors`

Your own: `text`, `custom` and `command`. Every line accepts `key`, `format`, `keyColor`, `valueColor`, `hide` and `when`. The config is real Lua, so lines can also be functions.

</details>

<sub>[↑ back to top](#nyfetch)</sub>

## 4.0 Roadmap

Still missing compared with Fastfetch: media player, sound, Wi-Fi, Bluetooth, brightness, gamepad, public IP, weather, GPU APIs, CPU usage, image logos and Fastfetch's JSON config. NyFetch was mostly developed and tested in a Linux VM, so some desktop-specific lines may differ on your hardware. Bug reports are welcome in [Issues](https://github.com/77nyan/nyfetch/issues).

<sub>[↑ back to top](#nyfetch)</sub>

## 5.0 Credits

NyFetch is [MIT](LICENSE) licensed. The logos and the layout idea come from [Fastfetch](https://github.com/fastfetch-cli/fastfetch) (MIT, see [`LICENSE-FASTFETCH`](LICENSE-FASTFETCH)). Distribution logos are trademarks of their respective owners.

<sub>[↑ back to top](#nyfetch)</sub>
