#!/bin/sh
# NyFetch installer
#   ./install.sh              install for you   (~/.local/bin, no sudo needed)
#   ./install.sh --system     install for everyone (/usr/local/bin, asks for sudo)
#   ./install.sh --uninstall  remove NyFetch (your config is kept)
set -e
cd "$(dirname "$0")"

# ---- looks ---------------------------------------------------------------------------
if [ -t 1 ]; then
  P='\033[35m'; G='\033[32m'; R='\033[31m'; D='\033[2m'; B='\033[1m'; N='\033[0m'
else
  P=''; G=''; R=''; D=''; B=''; N=''
fi
step() { printf "${P}${B}::${N} ${B}%s${N}\n" "$1"; }
ok()   { printf "   ${G}✓${N} %s\n" "$1"; }
note() { printf "   ${D}%s${N}\n" "$1"; }
die()  { printf "   ${R}✗ %s${N}\n" "$1"; exit 1; }

printf "\n  ${P}${B}NyFetch${N} ${D}installer${N}\n\n"

BIN_DIR="$HOME/.local/bin"
SUDO=""
case "$1" in
  "") ;;
  --system) BIN_DIR=/usr/local/bin; SUDO=sudo ;;
  --uninstall)
    step "Removing NyFetch"
    rm -f "$HOME/.local/bin/nyfetch"
    [ -e /usr/local/bin/nyfetch ] && sudo rm -f /usr/local/bin/nyfetch
    ok "Removed (your config in ~/.config/nyfetch was kept)"
    echo; exit 0 ;;
  *) die "unknown option: $1  (use --system or --uninstall)" ;;
esac

# ---- 1. tools ------------------------------------------------------------------------
step "Checking build tools"
if ! command -v cargo >/dev/null 2>&1 || ! command -v cc >/dev/null 2>&1; then
  if command -v pacman >/dev/null 2>&1; then
    note "Installing rust and gcc with pacman..."
    sudo pacman -S --needed --noconfirm rust gcc
  else
    die "Please install Rust (https://rustup.rs) and a C compiler (gcc or clang), then run again."
  fi
fi
ok "Rust and a C compiler found"

# ---- 2. build ------------------------------------------------------------------------
step "Building NyFetch ${D}(about a minute the first time)${N}"
cargo build --release --quiet
ok "Built"

# ---- 3. install ----------------------------------------------------------------------
step "Installing"
$SUDO mkdir -p "$BIN_DIR"
$SUDO install -m 755 target/release/nyfetch "$BIN_DIR/nyfetch"
ok "Installed to $BIN_DIR/nyfetch"

CFG="${XDG_CONFIG_HOME:-$HOME/.config}/nyfetch/config.lua"
if [ -f "$CFG" ]; then
  ok "Kept your existing config"
else
  mkdir -p "$(dirname "$CFG")"
  "$BIN_DIR/nyfetch" --gen-config > "$CFG"
  ok "Created your config: $CFG"
fi

# ---- 4. PATH -------------------------------------------------------------------------
add_line() {
  grep -qs "# added by NyFetch" "$1" && return 0
  printf '\n# added by NyFetch\nexport PATH="%s:$PATH"\n' "$BIN_DIR" >> "$1"
  ok "Added $BIN_DIR to your PATH in $1"
}
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    step "Adding $BIN_DIR to your PATH"
    case "$(basename "${SHELL:-sh}")" in
      fish)
        if command -v fish >/dev/null 2>&1; then
          fish -c "fish_add_path -U '$BIN_DIR'"
        else
          mkdir -p "$HOME/.config/fish/conf.d"
          echo "fish_add_path '$BIN_DIR'" > "$HOME/.config/fish/conf.d/nyfetch.fish"
        fi
        ok "Added $BIN_DIR to your fish PATH" ;;
      zsh)  add_line "$HOME/.zshrc" ;;
      bash) add_line "$HOME/.bashrc" ;;
      *)    add_line "$HOME/.profile" ;;
    esac
    note "Open a new terminal for this to take effect." ;;
esac

printf "\n  ${G}${B}Done!${N} Run ${B}nyfetch${N}, and edit ${B}%s${N} to make it yours.\n\n" "$CFG"
