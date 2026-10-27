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
sudo pacman -S --needed git rust gcc
git clone https://github.com/77nyan/nyfetch.git
cd nyfetch
./install.sh
```

The installer builds NyFetch, copies it to `~/.local/bin`, adds that folder to your `PATH`, and creates your config at `~/.config/nyfetch/config.lua`. Open a new terminal and run `nyfetch`.

**Other Linux distributions**: install Rust ([rustup.rs](https://rustup.rs)) and a C compiler (`gcc` or `clang`), then run the same steps.

**Dependencies**

| What | Why |
|------|-----|
| `rust` (cargo) | builds NyFetch |
| `gcc` | compiles the bundled Lua interpreter |
| `git` | only to download the source |

Nothing is needed to *run* it: the binary is fully static.

`./install.sh --system` installs for all users (`/usr/local/bin`), `./install.sh --uninstall` removes it (your config is kept).
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
