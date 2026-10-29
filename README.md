# NyFetch

A fast, lightweight system information tool for Linux, written in Rust. It looks like [Fastfetch](https://github.com/fastfetch-cli/fastfetch) (same layout, same logos), but you configure it with a simple **Lua** file that anyone can edit, even without coding experience.

<!-- Add a screenshot here: ![NyFetch](screenshot.png) -->

## Features

- **Fast and small**: one static binary, about 3 ms per run and about 3 MB of memory in my tests. Package counts are cached.
- **All of Fastfetch's logos** (540+), with small versions for many distros (`--small`).
- **Beginner-friendly Lua config**: hide, move or rename lines, change colors, add your own text. A tutorial is at the bottom of the config file.
- **Powerful when you want it**: the config is real Lua, so lines can be functions, run commands, draw bars and gradients, or build the whole layout.

## Install

**CachyOS / Arch**

```sh
sudo pacman -S --needed git
git clone https://github.com/77nyan/nyfetch.git
cd nyfetch
sudo ./install.sh
```

`sudo ./install.sh` is the only way to start the installer (`sh install.sh` will refuse). It opens a small menu; move with the arrow keys (or press `1`-`4`) and press Enter:

```
┌  NyFetch installer
│
●  NyFetch is not installed yet.
│  This folder has version 0.1.0.
│
◆  What would you like to do?
│  ● Install  (build NyFetch and install it for everyone)
│  ○ Update
│  ○ Uninstall
│  ○ Quit
└
```

| Option | What it does |
|--------|--------------|
| **Install** | Installs `rust` and `gcc` with pacman if they are missing, builds NyFetch, installs it to `/usr/local/bin/nyfetch` and creates your config at `~/.config/nyfetch/config.lua`. |
| **Update** | Downloads the newest version from GitHub (`git pull`), rebuilds and reinstalls. Your config is kept. |
| **Uninstall** | Removes NyFetch. It asks before touching your config. |
| **Quit** | Closes the installer. |

Afterwards just run `nyfetch` in any terminal. If the installer says "permission denied", run `chmod +x install.sh` once.

**Other Linux distributions**: install Rust ([rustup.rs](https://rustup.rs)) and a C compiler (`gcc` or `clang`) yourself, then run the same steps.

**Dependencies**

| What | Why |
|------|-----|
| `rust` (cargo) | builds NyFetch (the installer installs it on Arch) |
| `gcc` | compiles the bundled Lua interpreter (the installer installs it on Arch) |
| `git` | downloads the source and updates |

Nothing is needed to *run* it: the binary is fully static.
To build by hand: `cargo build --release`, then run `target/release/nyfetch`.

## Usage

```
nyfetch                   show your system
nyfetch --small           use the small logo
nyfetch -l arch           use another logo (or: random, none, or a text file)
nyfetch --list-logos      list all logos
nyfetch --gen-config      print the default config
nyfetch -c file.lua       use another config file
nyfetch --bench           show how long each line takes
nyfetch --help            all options
```

## Configure

Edit `~/.config/nyfetch/config.lua`. The top of the file has simple settings and the list of lines; the tutorial is at the bottom. For example:

```lua
local logo = "arch"          -- pick a logo
local small_logo = true      -- smaller logo

local lines = {
  "title",
  "os",
  "kernel",
  -- "uptime",                                -- hidden: -- in front of a line
  { type = "memory", key = "RAM" },           -- renamed
  { type = "text", text = "Hello {user}!" },  -- your own text
  "colors",
}
```

Available lines: `title separator os host kernel uptime packages shell display de wm theme icons font cursor terminal terminalfont cpu gpu memory swap disk localip battery locale board bios processes break colors`, plus `text`, `custom` and `command` for your own.

Every line can be changed with `key`, `format` (words in `{braces}`, e.g. `"{used} of {total}"`), `keyColor`, `valueColor`, `hide` and `when`. Colors accept names (`red`, `bright_blue`), `#rrggbb`, `0`-`255` and styles (`bold`, `dim`, `italic`...). Lua helpers for advanced configs: `exec`, `env`, `read_file`, `file_exists`, `bar`, `bytes`, `gradient`, `ansi`, `nyfetch.query`.

## Not done yet

Compared with Fastfetch, still missing: media player, sound, Wi-Fi, Bluetooth, brightness, gamepad, physical disks, public IP, weather, OpenGL/Vulkan/OpenCL, CPU usage, image logos (kitty/sixel) and Fastfetch's JSON config format. NyFetch has been developed and tested mostly in a Linux VM, so some desktop-specific lines may differ from Fastfetch on your hardware. Bug reports are welcome.

## Credits and license

NyFetch is MIT licensed (see `LICENSE`). The logos and the layout idea come from [Fastfetch](https://github.com/fastfetch-cli/fastfetch) (MIT, see `LICENSE-FASTFETCH`). Distribution logos are trademarks of their respective owners.
