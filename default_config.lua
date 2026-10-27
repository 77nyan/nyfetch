-- Go to the bottom of this file for a tutorial on how to customize it.

-- ============================== SETTINGS ==============================
local logo        = "auto"    -- "auto", "random", "none", or any name from: nyfetch --list-logos
local small_logo  = false     -- true = smaller logo
local logo_side   = "left"    -- "left", "right" or "top"
local logo_colors = {}        -- {} = normal colors, or e.g. { "cyan", "magenta" }
local key_color   = "auto"    -- color of the names on the left (OS, Kernel...)
local title_color = "auto"    -- color of the user@host line
local separator   = ": "      -- text between a name and its value
local bold        = true
local binary      = true      -- true = GiB/MiB   false = GB/MB
local decimals    = 2

-- ============================ WHAT TO SHOW ============================
-- Hide a line by putting -- in front of it. Move a line to change its order.
local lines = {
  "title",
  "separator",
  "os",
  "host",
  "kernel",
  "uptime",
  "packages",
  "shell",
  "display",
  "de",
  "wm",
  "theme",
  "icons",
  "font",
  "cursor",
  "terminal",
  "terminalfont",
  "cpu",
  "gpu",
  "memory",
  "swap",
  "disk",
  "localip",
  "battery",
  "locale",
  -- "board",
  -- "bios",
  -- "processes",
  "break",
  "colors",

  -- Your own lines go here (see the tutorial below):
  -- { type = "text", text = "Hello {user}!" },
  -- { type = "custom", key = "Mood", format = "Happy" },
}

-- ======================== (no need to change this) ========================
return {
  logo = { source = logo, size = small_logo and "small" or "normal", position = logo_side,
           color = logo_colors, padding = { top = 0, left = 0, right = 4, bottom = 1 } },
  display = { separator = separator, bold = bold, binary = binary, decimals = decimals,
              color = { keys = key_color, title = title_color } },
  modules = lines,
}

--[[ ===========================  TUTORIAL  ===========================

THE BASICS
  * A line starting with --  is a note. NyFetch ignores it.
    Put -- in front of a line to switch it off, remove the -- to switch it on.
  * Text goes in "quotes". Lines in a list end with a comma.
  * true = yes, false = no.
  * Save this file as ~/.config/nyfetch/config.lua, then run: nyfetch
  * Broke something? NyFetch shows the line to look at. To start over:
        nyfetch --gen-config > ~/.config/nyfetch/config.lua

CHANGE THE LOGO
  local logo = "arch"          any name from: nyfetch --list-logos
  local small_logo = true      smaller version (if the logo has one)
  local logo_colors = { "red", "white" }
  Colors: black red green yellow blue magenta cyan white (bright_red ...),
          orange, 0-255, or hex like "#ff8800".

HIDE, MOVE OR ADD LINES (in the "lines" list)
  Hide:   put -- in front of the line
  Move:   cut the line and paste it somewhere else
  Add:    remove the -- in front of board, bios or processes

CHANGE A LINE'S NAME, LOOK OR COLOR (paste into the "lines" list)
  { type = "memory", key = "RAM" },                                   new name
  { type = "memory", key = "RAM", format = "{used} of {total}" },     new look
  { type = "cpu", format = "{name} with {cores} cores" },
  { type = "kernel", keyColor = "yellow" },                           name color
  { type = "memory", valueColor = "bold cyan" },                      value color
  { type = "disk", paths = { "/", "/home" } },                        only these drives
  { type = "disk", paths = { "/", "/boot/efi" } },                    show the boot drive
  { type = "gpu", hide = true },                                      switch off

ADD YOUR OWN LINES
  { type = "text", text = "Hello {user}!" },                  plain text
  { type = "custom", key = "Mood", format = "{#green}Happy{#}" },   key: value
  { type = "command", key = "Today", cmd = "date +%A" },      output of a command
  { type = "break" },                                         empty line
  { type = "colors", symbol = "●", width = 2 },               round color dots

  In text you can use:  {user}  {host}  {env:HOME}
  and colors:           {#red} ... {#}   (color on ... color off)

WORDS YOU CAN USE IN format = "..."  (put in {braces})
  os        name arch id version codename      cpu     name cores freq
  host      name version vendor                gpu     vendor name type freq
  kernel    sysname release arch               memory  used total percent
  uptime    text days hours minutes            swap    used total percent
  packages  all total (pacman dpkg rpm ...)    disk    path used total percent fs
  shell     name version path                  battery model percent status
  display   name width height refresh type     localip name ip prefix
  terminal  name version                       terminalfont name size
  wm        name version protocol              (full list: README on GitHub)

ADVANCED (code)
  A line made with code, here a RAM bar:
    function()
      local m = nyfetch.query("memory")[1]
      return "RAM " .. bar(m.percent_num, 20) .. " " .. m.percent
    end,
  Show a line only on laptops:
    { type = "battery", when = function() return file_exists("/sys/class/power_supply/BAT0") end },
  Rainbow title:
    function() return gradient(nyfetch.user .. "@" .. nyfetch.host, "#ff5f6d", "#ffc371") end,
  Helpers: exec("cmd") env("NAME") read_file(path) file_exists(path) ansi("bold red")
           bar(pct, width) bytes(n) gradient(text, a, b) width(s) strip(s)
           nyfetch.query("cpu") nyfetch.user nyfetch.host
  You can also take over the whole layout with  render = function(logo, info, ctx) ... end
  and split the config into files with require("otherfile").

=============================================================================== ]]
