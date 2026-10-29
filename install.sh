#!/bin/bash
# NyFetch installer.  Start it with:   sudo ./install.sh
#
#   1) Install    build NyFetch and install it for everyone (/usr/local/bin)
#   2) Update     get the newest version, rebuild and reinstall (your config is kept)
#   3) Uninstall  remove NyFetch
#   4) Quit
#
# The look follows the AerynOS "Lichen" installer (clack-style prompts).

# ============================================================================================
#  Part 1: the safety check.  Plain POSIX shell on purpose, so that even `sh install.sh`
#  (dash, busybox...) can read it and print the message instead of failing in a strange way.
# ============================================================================================
ESC=$(printf '\033')
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  C_GRAY="${ESC}[90m"; C_RED="${ESC}[31m"; C_BOLD="${ESC}[1m"; C_RST="${ESC}[0m"
else
  C_GRAY=""; C_RED=""; C_BOLD=""; C_RST=""
fi

refuse() {
  printf '\n%s┌%s  %sNyFetch installer%s\n' "$C_GRAY" "$C_RST" "$C_BOLD" "$C_RST"
  printf '%s│%s\n' "$C_GRAY" "$C_RST"
  printf '%s■%s  %s\n' "$C_RED" "$C_RST" "$1"
  printf '%s│%s\n' "$C_GRAY" "$C_RST"
  printf '%s│%s  Start it from your normal account like this:\n' "$C_GRAY" "$C_RST"
  printf '%s│%s\n' "$C_GRAY" "$C_RST"
  printf '%s│%s      %ssudo ./install.sh%s\n' "$C_GRAY" "$C_RST" "$C_BOLD" "$C_RST"
  printf '%s│%s\n' "$C_GRAY" "$C_RST"
  printf '%s└%s  %sNothing was changed.%s\n\n' "$C_GRAY" "$C_RST" "$C_RED" "$C_RST"
  exit 1
}

case "$0" in
  */*) ;;
  *) refuse "This installer cannot be started with \"sh\" or \"bash\"." ;;
esac
[ "$(id -u)" -eq 0 ] || refuse "The installer needs administrator rights."
if [ -z "${SUDO_USER:-}" ] || [ "$SUDO_USER" = "root" ]; then
  [ -n "${NYFETCH_ALLOW_ROOT:-}" ] || refuse "Please do not run it from a root shell."
fi
[ -n "${BASH_VERSION:-}" ] || refuse "This installer needs bash."

# ============================================================================================
#  Part 2: look and feel
# ============================================================================================
if [ -n "${NYFETCH_ASCII:-}" ]; then UTF=0
else
  UTF=1
  case "${LC_ALL:-${LC_CTYPE:-${LANG:-}}}" in
    ""|*[Uu][Tt][Ff]-8*|*[Uu][Tt][Ff]8*) ;;
    *) UTF=0 ;;
  esac
  [ "${TERM:-}" = "linux" ] && UTF=0
fi

if [ "$UTF" = 1 ]; then
  S_START="┌"; S_BAR="│"; S_END="└"; S_ACTIVE="◆"; S_DONE="◇"; S_CANCEL="■"; S_WARN="▲"; S_INFO="●"
  S_ON="●"; S_OFF="○"; S_H="─"; S_TR="╮"; S_BR="╯"; S_TEE="├"
  FRAMES=("◒" "◐" "◓" "◑")
else
  S_START="T"; S_BAR="|"; S_END="-"; S_ACTIVE="*"; S_DONE="o"; S_CANCEL="x"; S_WARN="!"; S_INFO="*"
  S_ON=">"; S_OFF=" "; S_H="-"; S_TR="+"; S_BR="+"; S_TEE="+"
  FRAMES=("|" "/" "-" "\\")
fi

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  GRAY="${ESC}[90m"; CYAN="${ESC}[36m"; GREEN="${ESC}[32m"; RED="${ESC}[31m"; YELLOW="${ESC}[33m"
  BLUE="${ESC}[34m"; MAGENTA="${ESC}[35m"; BOLD="${ESC}[1m"; DIM="${ESC}[2m"; RST="${ESC}[0m"
else
  GRAY=""; CYAN=""; GREEN=""; RED=""; YELLOW=""; BLUE=""; MAGENTA=""; BOLD=""; DIM=""; RST=""
fi
INTERACTIVE=0
[ -t 0 ] && [ -t 1 ] && INTERACTIVE=1

bar()   { printf '%s%s%s\n' "$GRAY" "$S_BAR" "$RST"; }
intro() { printf '\n%s%s%s  %s%s%s\n' "$GRAY" "$S_START" "$RST" "$BOLD" "$1" "$RST"; bar; }
outro() { printf '%s%s%s  %s\n\n' "$GRAY" "$S_END" "$RST" "$1"; }
outro_cancel() { printf '%s%s%s  %s%s%s\n\n' "$GRAY" "$S_END" "$RST" "$RED" "$1" "$RST"; }

# log SYMBOL_COLOR SYMBOL "text"  (extra lines of text get the side bar)
log_line() {
  local color=$1 sym=$2 text first=1 line
  text=$(printf '%b' "$3")
  while IFS= read -r line; do
    if [ $first = 1 ]; then printf '%s%s%s  %s\n' "$color" "$sym" "$RST" "$line"; first=0
    else printf '%s%s%s  %s\n' "$GRAY" "$S_BAR" "$RST" "$line"; fi
  done <<EOF
$text
EOF
  bar
}
log_info()  { log_line "$BLUE" "$S_INFO" "$1"; }
log_warn()  { log_line "$YELLOW" "$S_WARN" "$1"; }
log_error() { log_line "$RED" "$S_CANCEL" "$1"; }
log_ok()    { log_line "$GREEN" "$S_DONE" "$1"; }

# note "Title" "line1\nline2"   draws the little box used for summaries
note() {
  local title=$1 text=$2 width=${#1} line
  while IFS= read -r line; do [ ${#line} -gt "$width" ] && width=${#line}; done <<EOF
$text
EOF
  width=$((width + 2))
  printf '%s%s%s  %s %s%s%s\n' "$GREEN" "$S_DONE" "$RST" "$title" "$GRAY" "$(rep "$S_H" $((width - ${#title})))$S_TR" "$RST"
  printf '%s%s%s%*s%s%s%s\n' "$GRAY" "$S_BAR" "$RST" $((width + 3)) "" "$GRAY" "$S_BAR" "$RST"
  while IFS= read -r line; do
    printf '%s%s%s  %s%*s%s%s%s\n' "$GRAY" "$S_BAR" "$RST" "$line" $((width - ${#line} + 1)) "" "$GRAY" "$S_BAR" "$RST"
  done <<EOF
$text
EOF
  printf '%s%s%s%*s%s%s%s\n' "$GRAY" "$S_BAR" "$RST" $((width + 3)) "" "$GRAY" "$S_BAR" "$RST"
  printf '%s%s%s%s\n' "$GRAY" "$S_TEE" "$(rep "$S_H" $((width + 3)))$S_BR" "$RST"
  bar
}
rep() { local out="" i; for ((i = 0; i < $2; i++)); do out+="$1"; done; printf '%s' "$out"; }

CURSOR_HIDDEN=0
hide_cursor() { [ $INTERACTIVE = 1 ] && { printf '%s[?25l' "$ESC"; CURSOR_HIDDEN=1; }; }
show_cursor() { [ $CURSOR_HIDDEN = 1 ] && { printf '%s[?25h' "$ESC"; CURSOR_HIDDEN=0; }; }
STEP_LOG=""
cleanup() { show_cursor; [ -n "$STEP_LOG" ] && rm -f "$STEP_LOG"; }
cancelled() { show_cursor; printf '\n'; bar; outro_cancel "Cancelled. Nothing else was changed."; exit 130; }
trap cleanup EXIT
trap cancelled INT TERM

# read one key: sets KEY to up, down, left, right, enter, esc, or the character itself
read_key() {
  local k rest
  IFS= read -rsn1 k || { KEY=esc; return; }
  case "$k" in
    "") KEY=enter ;;
    "$ESC")
      rest=""
      IFS= read -rsn2 -t 0.1 rest
      case "$rest" in
        "[A") KEY=up ;; "[B") KEY=down ;; "[C") KEY=right ;; "[D") KEY=left ;;
        *) KEY=esc ;;
      esac ;;
    *) KEY=$k ;;
  esac
}

# select "Question" "Label|hint" ...      -> SEL = chosen index (0-based), or -1 when cancelled
select_menu() {
  local title=$1; shift
  local items=("$@") n=$# cur=0 i label hint lines=$(($# + 2))
  if [ $INTERACTIVE = 0 ]; then
    printf '%s\n' "$title" >&2
    for ((i = 0; i < n; i++)); do printf '  %d) %s\n' $((i + 1)) "${items[i]%%|*}" >&2; done
    printf 'Choose 1-%d: ' "$n" >&2
    read -r SEL || SEL=0
    case "$SEL" in [1-9]) SEL=$((SEL - 1)) ;; *) SEL=-1 ;; esac
    [ "$SEL" -ge "$n" ] && SEL=-1
    return
  fi
  draw() {
    printf '%s%s%s  %s\n' "$CYAN" "$S_ACTIVE" "$RST" "$title"
    for ((i = 0; i < n; i++)); do
      label=${items[i]%%|*}; hint=""
      [[ ${items[i]} == *"|"* ]] && hint=${items[i]#*|}
      if [ $i = $cur ]; then
        printf '%s%s%s  %s%s%s %s%s%s%s\n' "$CYAN" "$S_BAR" "$RST" "$GREEN" "$S_ON" "$RST" "$label" "$DIM" "${hint:+  ($hint)}" "$RST"
      else
        printf '%s%s%s  %s%s %s%s\n' "$CYAN" "$S_BAR" "$RST" "$DIM" "$S_OFF" "$label" "$RST"
      fi
    done
    printf '%s%s%s\n' "$CYAN" "$S_END" "$RST"
  }
  hide_cursor
  draw
  while :; do
    read_key
    case "$KEY" in
      up|k)   cur=$(((cur + n - 1) % n)) ;;
      down|j) cur=$(((cur + 1) % n)) ;;
      [1-9])  if [ "$KEY" -le "$n" ]; then cur=$((KEY - 1)); KEY=enter; fi ;;
      q|esc)  cur=-1; KEY=enter ;;
    esac
    printf '%s[%dA%s[J' "$ESC" "$lines" "$ESC"
    if [ "$KEY" = enter ]; then
      if [ $cur -ge 0 ]; then
        printf '%s%s%s  %s\n%s%s%s  %s%s%s\n' "$GREEN" "$S_DONE" "$RST" "$title" "$GRAY" "$S_BAR" "$RST" "$DIM" "${items[cur]%%|*}" "$RST"
        bar
      else
        printf '%s%s%s  %s\n' "$RED" "$S_CANCEL" "$RST" "$title"
      fi
      show_cursor
      SEL=$cur
      return
    fi
    draw
  done
}

# confirm "Question" [default yes|no]   -> returns 0 for yes, 1 for no
confirm() {
  local title=$1 cur=1 ans
  [ "${2:-yes}" = no ] && cur=0
  if [ $INTERACTIVE = 0 ]; then [ "${2:-yes}" = yes ]; return; fi
  draw() {
    local yes no
    if [ $cur = 1 ]; then yes="${GREEN}${S_ON}${RST} Yes"; no="${DIM}${S_OFF} No${RST}"
    else yes="${DIM}${S_OFF} Yes${RST}"; no="${GREEN}${S_ON}${RST} No"; fi
    printf '%s%s%s  %s\n%s%s%s  %s %s/%s %s\n%s%s%s\n' "$CYAN" "$S_ACTIVE" "$RST" "$title" \
      "$CYAN" "$S_BAR" "$RST" "$yes" "$DIM" "$RST" "$no" "$CYAN" "$S_END" "$RST"
  }
  hide_cursor
  draw
  while :; do
    read_key
    case "$KEY" in
      left|right|up|down|h|l|k|j) cur=$((1 - cur)) ;;
      y|Y) cur=1; KEY=enter ;;
      n|N) cur=0; KEY=enter ;;
      esc) cur=0; KEY=enter ;;
    esac
    printf '%s[3A%s[J' "$ESC" "$ESC"
    if [ "$KEY" = enter ]; then
      ans="No"; [ $cur = 1 ] && ans="Yes"
      printf '%s%s%s  %s\n%s%s%s  %s%s%s\n' "$GREEN" "$S_DONE" "$RST" "$title" "$GRAY" "$S_BAR" "$RST" "$DIM" "$ans" "$RST"
      bar
      show_cursor
      [ $cur = 1 ]
      return
    fi
    draw
  done
}

# step "Doing..." "Done." command args...   runs a command with a spinner; stops the installer on failure
step() {
  local msg=$1 done_msg=$2 start=$SECONDS pid rc i=0 secs frame
  shift 2
  STEP_LOG=$(mktemp)
  "$@" >"$STEP_LOG" 2>&1 &
  pid=$!
  if [ $INTERACTIVE = 1 ]; then
    hide_cursor
    while kill -0 "$pid" 2>/dev/null; do
      frame=${FRAMES[i % ${#FRAMES[@]}]}
      printf '\r%s%s%s  %s' "$MAGENTA" "$frame" "$RST" "$msg"
      i=$((i + 1)); sleep 0.1
    done
    show_cursor
  fi
  wait "$pid"; rc=$?
  secs=$((SECONDS - start))
  [ $INTERACTIVE = 1 ] && printf '\r%s[K' "$ESC"
  if [ $rc = 0 ]; then
    printf '%s%s%s  %s' "$GREEN" "$S_DONE" "$RST" "$done_msg"
    [ $secs -ge 2 ] && printf '  %s(%ss)%s' "$DIM" "$secs" "$RST"
    printf '\n'
    bar
    return 0
  fi
  if [ "${SOFT:-0}" = 1 ]; then
    printf '%s%s%s  %s did not work\n' "$YELLOW" "$S_WARN" "$RST" "$msg"
    tail -n 4 "$STEP_LOG" | while IFS= read -r line; do printf '%s%s%s  %s%s%s\n' "$GRAY" "$S_BAR" "$RST" "$DIM" "$line" "$RST"; done
    bar
    return 1
  fi
  printf '%s%s%s  %s failed\n' "$RED" "$S_CANCEL" "$RST" "$msg"
  tail -n 15 "$STEP_LOG" | while IFS= read -r line; do printf '%s%s%s  %s%s%s\n' "$GRAY" "$S_BAR" "$RST" "$DIM" "$line" "$RST"; done
  bar
  outro_cancel "The installer stopped. Your system was not changed any further."
  exit 1
}

# ============================================================================================
#  Part 3: what the installer knows about this machine
# ============================================================================================
# sudo sometimes starts scripts with a short PATH: make sure the system tools can be found
PATH="$PATH:/usr/local/sbin:/usr/sbin:/sbin"
SRC_DIR=$(cd "$(dirname "$0")" && pwd)
TARGET_USER=${SUDO_USER:-root}
USER_HOME=$(getent passwd "$TARGET_USER" | cut -d: -f6)
[ -n "$USER_HOME" ] || USER_HOME=$(eval echo "~$TARGET_USER")
BIN=/usr/local/bin/nyfetch
LEGACY_BIN="$USER_HOME/.local/bin/nyfetch"
CFG_DIR="$USER_HOME/.config/nyfetch"
CFG="$CFG_DIR/config.lua"
USER_PATH="$USER_HOME/.cargo/bin:$PATH"

as_user() {
  if command -v runuser >/dev/null 2>&1; then
    runuser -u "$TARGET_USER" -- env HOME="$USER_HOME" PATH="$USER_PATH" "$@"
  else
    sudo -u "$TARGET_USER" -H env PATH="$USER_PATH" "$@"
  fi
}
installed_version() { [ -x "$BIN" ] && "$BIN" -V 2>/dev/null | sed 's/^nyfetch //'; }
source_version()    { sed -n 's/^version = "\(.*\)"/\1/p' "$SRC_DIR/Cargo.toml" | head -n 1; }
have_build_tools()  { as_user sh -c 'command -v cargo >/dev/null 2>&1 && command -v cc >/dev/null 2>&1'; }

# ============================================================================================
#  Part 4: the actions
# ============================================================================================
install_tools()  { pacman -S --needed --noconfirm rust gcc; }
build()          { cd "$SRC_DIR" || exit 1; [ -d target ] && chown -R "$TARGET_USER" target; as_user cargo build --release; }
put_in_place()   { install -Dm755 "$SRC_DIR/target/release/nyfetch" "$BIN" && rm -f "$LEGACY_BIN"; }
pull_updates()   { as_user git -C "$SRC_DIR" pull --ff-only; }
make_config()    { as_user mkdir -p "$CFG_DIR" && as_user "$BIN" --gen-config | as_user tee "$CFG" >/dev/null; }

check_system() {
  if [ "$(uname -s)" != "Linux" ]; then
    log_error "NyFetch only works on Linux."; outro_cancel "Nothing was changed."; exit 1
  fi
  if [ ! -f "$SRC_DIR/Cargo.toml" ]; then
    log_error "I cannot find the NyFetch source next to install.sh.\nRun it from inside the downloaded nyfetch folder."
    outro_cancel "Nothing was changed."; exit 1
  fi
}

ensure_tools() {
  if have_build_tools; then return; fi
  if command -v pacman >/dev/null 2>&1; then
    step "Installing build tools (rust, gcc)" "Installed build tools" install_tools
  else
    log_error "Rust and a C compiler are needed to build NyFetch.\nInstall Rust (https://rustup.rs) and gcc or clang, then run the installer again."
    outro_cancel "Nothing was changed."; exit 1
  fi
}

# Keep the user's config when it works; replace it when it is missing, empty or broken.
setup_config() {
  if [ -s "$CFG" ] && as_user "$BIN" -c "$CFG" --pipe >/dev/null 2>&1; then
    CFG_NOTE="kept your existing config"
    return
  fi
  if [ -s "$CFG" ]; then
    mv "$CFG" "$CFG.bak" && chown "$TARGET_USER" "$CFG.bak"
    CFG_NOTE="new config, the old one is saved as config.lua.bak"
  else
    CFG_NOTE="new config created"
  fi
  step "Creating your config" "Created your config" make_config
}

show_result() {
  local new=$1 headline=$2
  note "$headline" "$(printf 'Version   %s\nProgram   %s\nConfig    %s  (%s)\n\nRun it:   nyfetch' "$new" "$BIN" "$CFG" "$CFG_NOTE")"
}

do_install() {
  local old new
  old=$(installed_version)
  check_system
  if [ -n "$old" ]; then
    log_warn "NyFetch $old is already installed.\nI will rebuild it and install it again."
  fi
  ensure_tools
  step "Building NyFetch (about a minute the first time)" "Built NyFetch" build
  step "Installing to $BIN" "Installed to $BIN" put_in_place
  setup_config
  new=$(installed_version)
  show_result "$new" "NyFetch is ready"
  outro "Enjoy! Open ${CFG/#$USER_HOME/\~} to make it yours."
}

do_update() {
  local old new before after
  old=$(installed_version)
  check_system
  if [ -z "$old" ]; then
    log_warn "NyFetch is not installed yet, so I will install it."
    do_install; return
  fi
  before=$(as_user git -C "$SRC_DIR" rev-parse --short HEAD 2>/dev/null)
  if [ -d "$SRC_DIR/.git" ] && command -v git >/dev/null 2>&1; then
    if SOFT=1 step "Checking GitHub for a newer version" "Checked GitHub" pull_updates; then
      after=$(as_user git -C "$SRC_DIR" rev-parse --short HEAD 2>/dev/null)
      if [ "$before" = "$after" ]; then log_info "Already on the newest source ($after)."
      else log_info "Downloaded new changes ($before -> $after)."; fi
    else
      log_warn "Could not download updates (no internet, or you changed files here).\nI will rebuild what is in this folder instead."
    fi
  else
    log_info "This folder is not a git checkout, so I will rebuild what is here.\nTo get new versions, download NyFetch again with git clone."
  fi
  ensure_tools
  step "Building NyFetch" "Built NyFetch" build
  step "Installing to $BIN" "Installed to $BIN" put_in_place
  setup_config
  new=$(installed_version)
  if [ "$old" = "$new" ]; then show_result "$new" "NyFetch is up to date"
  else show_result "$new" "Updated NyFetch $old -> $new"; fi
  outro "All done."
}

remove_path_lines() {
  local f
  for f in "$USER_HOME/.bashrc" "$USER_HOME/.zshrc" "$USER_HOME/.profile"; do
    [ -f "$f" ] && sed -i '/^# added by NyFetch$/,+1d' "$f"
  done
  rm -f "$USER_HOME/.config/fish/conf.d/nyfetch.fish"
  return 0
}
remove_files() {
  rm -f "$BIN" "$LEGACY_BIN"
  rm -rf "$USER_HOME/.cache/nyfetch"
  remove_path_lines
}
remove_config() { rm -rf "$CFG_DIR"; }

do_uninstall() {
  local old=$(installed_version) delete_cfg=0
  if [ -z "$old" ] && [ ! -e "$LEGACY_BIN" ]; then
    log_info "NyFetch is not installed, so there is nothing to remove."
    outro "Nothing to do."; return
  fi
  if ! confirm "Remove NyFetch ${old} from this computer?" yes; then
    outro_cancel "Cancelled. Nothing was changed."; return
  fi
  if [ -d "$CFG_DIR" ] && confirm "Also delete your settings (${CFG_DIR/#$USER_HOME/\~})?" no; then delete_cfg=1; fi
  step "Removing NyFetch" "Removed NyFetch" remove_files
  if [ $delete_cfg = 1 ]; then
    step "Deleting your settings" "Deleted your settings" remove_config
    outro "NyFetch is gone, along with your settings."
  elif [ -d "$CFG_DIR" ]; then
    outro "NyFetch is gone. Your settings are still in ${CFG_DIR/#$USER_HOME/\~}."
  else
    outro "NyFetch is gone."
  fi
}

# ============================================================================================
#  Part 5: start
# ============================================================================================
main() {
  local choice=${1:-}
  intro "NyFetch installer"
  cur_ver=$(installed_version)
  if [ -n "$cur_ver" ]; then log_info "NyFetch $cur_ver is installed.\nThis folder has version $(source_version)."
  else log_info "NyFetch is not installed yet.\nThis folder has version $(source_version)."; fi

  if [ -z "$choice" ]; then
    if [ $INTERACTIVE = 0 ]; then
      log_error "This needs a terminal.\nOr choose directly:  sudo ./install.sh install | update | uninstall"
      outro_cancel "Nothing was changed."; exit 1
    fi
    select_menu "What would you like to do?" \
      "Install|build NyFetch and install it for everyone" \
      "Update|get the newest version and reinstall" \
      "Uninstall|remove NyFetch" \
      "Quit|"
    case $SEL in 0) choice=install ;; 1) choice=update ;; 2) choice=uninstall ;; *) choice=quit ;; esac
  fi

  case "$choice" in
    install|1)   do_install ;;
    update|2)    do_update ;;
    uninstall|3) do_uninstall ;;
    quit|4)      outro "Bye!" ;;
    *) log_error "Unknown choice \"$choice\".\nUse:  sudo ./install.sh [install|update|uninstall]"; outro_cancel "Nothing was changed."; exit 1 ;;
  esac
}
main "$@"
