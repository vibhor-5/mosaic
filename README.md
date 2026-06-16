# Mosaic 🧩

Mosaic is a blazing fast, natively integrated macOS Tiling Window Manager (TWM) built in Rust. 

Drawing heavy inspiration from **i3** and **AeroSpace**, Mosaic intercepts macOS's native window management and forces a dynamic Binary Space Partitioning (BSP) layout, maximizing your screen real estate and entirely eliminating the need for a mouse.

## Features

- **i3-style Binding Modes**: No more claw-hand modifiers. Press `Option + R` to enter Resize Mode, use standard keys to adjust windows, and press `Escape` to return to normal.
- **The Scratchpad**: Instantly banish any window (like a terminal) to a hidden workspace, and summon it as a floating overlay over any monitor with a single keystroke.
- **Smart Gaps**: Beautiful padding around your windows. When only one window is present, the gaps dynamically disappear to give you 100% fullscreen real estate.
- **Global Window Marks**: Tag specific windows with a character and instantly jump to them across spaces.
- **Scripting Addition (SA)**: Bypasses macOS SIP limitations by injecting a Mach IPC proxy into the Dock, allowing instant, native window movement between Spaces without flashing screens.
- **No External Daemons**: Mosaic handles global hotkeys natively. No need for `skhd` or external hotkey daemons.

---

## Installation

### Prerequisites
- macOS 13.0+
- Rust toolchain (`cargo`)
- **System Integrity Protection (SIP)**: Must be partially disabled (`csrutil enable --without fs --without debug`) if you wish to use the Scripting Addition to move windows instantly between macOS Spaces.

### Install via Homebrew (Recommended)
You can easily install Mosaic without compiling from source using our official tap:

```bash
brew install vibhorkumar/tap/mosaic
```

### Build from Source
If you prefer to compile manually:
```bash
git clone https://github.com/vibhorkumar/mosaic.git
cd mosaic
make build
make install
```

### Injecting the Scripting Addition (Optional but Recommended)
To allow Mosaic to bypass `SkyLight` entitlement restrictions and move windows between Spaces flawlessly:
```bash
./scripts/install-sa.sh
```

### Running the Daemon
Mosaic runs as a standard LaunchDaemon or in your terminal:
```bash
mosaic
```
*Note: You must grant your terminal (or the binary) **Accessibility permissions** in `System Settings -> Privacy & Security -> Accessibility`.*

---

## Configuration

Mosaic reads its configuration from `~/.config/mosaic/mosaic.toml`. If the file does not exist, it falls back to sensible defaults.

### Example `mosaic.toml`

```toml
[general]
# The default layout engine. Options: "bsp", "monocle"
layout = "bsp"

[gaps]
# Inner padding between adjacent windows
inner = 8
# Outer padding between windows and the screen edge
outer = 8
# When enabled, a single window will drop its outer gaps to fill the screen
smart_gaps_on_monocle = true

# Application-specific rules
[[rules]]
app_name = "System Settings"
action = "Float"

[[rules]]
app_name = "Calculator"
action = "Float"

[[rules]]
title_contains = "Picture in Picture"
action = "Float"
```

---

## Keybindings

Mosaic uses `Option` (Alt) as its primary modifier key. 

### Main Mode
- `Option + H/J/K/L` - Focus West / South / North / East
- `Option + E` - Set layout to **BSP** (Tiling)
- `Option + W` - Set layout to **Monocle** (Fullscreen stacked)
- `Option + Minus (-)` - Toggle active window to/from the **Scratchpad**
- `Option + R` - Enter **Resize Mode**

### Resize Mode
Once in Resize Mode, standard keys manipulate the window without holding modifiers:
- `H / J / K / L` - Adjust split ratios to shrink/grow the active window
- `Escape` / `Enter` - Exit Resize Mode

---

## Architecture

1. **Window Tracker**: Uses CoreGraphics (`CGWindowList`) and the macOS Accessibility API (`AXUIElement`) to build an internal state tree of all active windows.
2. **Layout Engine**: Pure math BSP implementation that calculates exact geometric bounding boxes and applies them via AXAPI.
3. **IPC Server**: Listens on a Unix socket (`/tmp/mosaic.sock`) allowing the lightweight `mosaic-msg` CLI to control the daemon remotely.
4. **Mach IPC (MIG)**: The C payload injected into the Dock communicates with the Rust daemon over high-speed Mach messages to proxy private SkyLight APIs.

## Testing
Mosaic contains a robust E2E test suite. To run the tests:
```bash
cargo test
```
