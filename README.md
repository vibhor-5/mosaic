# Mosaic

A fast, keyboard-driven tiling window manager for macOS, written in Rust.
Mosaic utilizes the macOS Accessibility API for window management and provides deep integration with native macOS Spaces.

## Features
- **Fast and Lightweight**: Written in Rust for minimal latency.
- **Native Spaces Integration**: Uses macOS native spaces rather than emulating virtual workspaces.
- **Multiple Layouts**: BSP (Binary Space Partitioning), Monocle, and Master-Stack modes.
- **Rules Engine**: Float or ignore specific applications via configuration.
- **Daemon-based**: Runs completely in the background via `launchd`.
- **IPC Client**: Control the daemon instantly using the `mosaic-msg` CLI.

## Prerequisites
- macOS 10.15 or later.
- Rust toolchain (`cargo`).
- Accessibility Permissions: You must grant Terminal/Mosaic access in **System Settings > Privacy & Security > Accessibility**.

## Installation

We provide a simple `Makefile` for building and installing Mosaic.

```sh
# 1. Build the release binaries and install the launchd service
make install

# 2. Start the daemon (if not already started)
make start
```

## Usage

Use the `mosaic-msg` command to interact with the daemon. We recommend binding these commands to hotkeys using a tool like `skhd`.

```sh
mosaic-msg focus west
mosaic-msg space 2
mosaic-msg layout bsp
mosaic-msg resize west 0.05
```

For a full list of commands, run:
```sh
mosaic-msg --help
```

## Configuration

Mosaic reads its configuration from `~/.config/mosaic/config.toml`. A default configuration will be created for you upon the first run.

```toml
[gaps]
inner = 10
outer = 20

[[rules]]
app_name = "System Settings"
action = "float"

[[rules]]
app_name = "Calculator"
action = "float"
```

## Architecture & "Own API" 

macOS does not provide public APIs for directly moving windows across Spaces instantly. To achieve this, Mosaic connects to the `SkyLight.framework` (the WindowServer's private API). In the future, a Scripting Addition (payload injected into Dock.app) will act as our "own API" proxy to bypass the `com.apple.private.sky-light.universal-owner` entitlement restriction, allowing seamless space manipulation without SIP (System Integrity Protection) warnings.
