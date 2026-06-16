//! IPC (Inter-Process Communication) module for Mosaic.
//!
//! Implements a Unix Domain Socket server that listens for commands from the
//! `mosaic-msg` CLI tool or external hotkey daemons like `skhd`.
//!
//! ## Protocol
//!
//! The protocol is simple line-based text:
//! - Client sends a single line command (e.g., `focus east\n`)
//! - Server processes the command and responds with `ok\n` or `error: <message>\n`

use log::{debug, error, info, warn};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};

use crate::layout::{Direction, LayoutMode, SplitDirection};
use crate::Mosaic;

/// The IPC server that listens for commands on a Unix socket.
pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    /// Create a new IPC server bound to the given socket path.
    ///
    /// Removes any existing socket file at the path before binding.
    pub fn new(socket_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Remove stale socket file if it exists
        let _ = std::fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path)?;
        info!("IPC server listening on {}", socket_path);

        Ok(Self { listener })
    }

    /// Run the IPC server loop, processing commands against the shared state.
    ///
    /// This blocks the current thread.
    pub fn run(self, state: Arc<Mutex<Mosaic>>) -> Result<(), Box<dyn std::error::Error>> {
        for stream in self.listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let reader = BufReader::new(stream.try_clone()?);
                    for line in reader.lines() {
                        match line {
                            Ok(cmd) => {
                                let cmd = cmd.trim().to_string();
                                if cmd.is_empty() {
                                    continue;
                                }
                                debug!("IPC received: {}", cmd);
                                let response = handle_command(&cmd, &state);
                                let _ = stream.write_all(response.as_bytes());
                                let _ = stream.write_all(b"\n");
                                let _ = stream.flush();
                            }
                            Err(e) => {
                                warn!("IPC read error: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("IPC accept error: {}", e);
                }
            }
        }
        Ok(())
    }
}

/// Parse and execute an IPC command, returning a response string.
fn handle_command(cmd: &str, state: &Arc<Mutex<Mosaic>>) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return "error: empty command".to_string();
    }

    match parts[0] {
        "focus" => handle_focus(&parts[1..], state),
        "swap" => handle_swap(&parts[1..], state),
        "move-to-space" => handle_move_to_space(&parts[1..], state),
        "space" => handle_switch_space(&parts[1..], state),
        "layout" => handle_layout(&parts[1..], state),
        "toggle" => handle_toggle(&parts[1..], state),
        "rotate" => handle_rotate(&parts[1..], state),
        "equalize" => handle_equalize(&parts[1..], state),
        "resize" => handle_resize(&parts[1..], state),
        "retile" => handle_retile(state),
        "query" => handle_query(&parts[1..], state),
        "quit" => {
            info!("Received quit command");
            std::process::exit(0);
        }
        _ => format!("error: unknown command '{}'", parts[0]),
    }
}

fn handle_focus(args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    if args.is_empty() {
        return "error: focus requires a direction (north|south|east|west)".to_string();
    }
    let direction = match parse_direction(args[0]) {
        Some(d) => d,
        None => return format!("error: invalid direction '{}'", args[0]),
    };

    let mut s = state.lock().unwrap();
    let active_space = s.skylight.get_active_space(s.connection_id);
    if let Some(focused) = s.tracker.get_focused().map(|w| w.id) {
        let layout = s.layout_for_space(active_space);
        if let Some(target) = layout.focus_direction(focused, direction) {
            s.tracker.set_focused(target);
            // TODO: Actually raise and focus the target window via AX API
            return format!("ok: focused window {}", target);
        }
    }
    "error: no window in that direction".to_string()
}

fn handle_swap(args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    if args.is_empty() {
        return "error: swap requires a direction (north|south|east|west)".to_string();
    }
    let direction = match parse_direction(args[0]) {
        Some(d) => d,
        None => return format!("error: invalid direction '{}'", args[0]),
    };

    let mut s = state.lock().unwrap();
    let active_space = s.skylight.get_active_space(s.connection_id);
    if let Some(focused) = s.tracker.get_focused().map(|w| w.id) {
        let layout = s.layout_for_space(active_space);
        if let Some(target) = layout.focus_direction(focused, direction) {
            layout.swap_windows(focused, target);
            s.retile_space(active_space);
            return format!("ok: swapped {} with {}", focused, target);
        }
    }
    "error: no window to swap with in that direction".to_string()
}

fn handle_move_to_space(args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    if args.is_empty() {
        return "error: move-to-space requires a space number".to_string();
    }
    let space_idx: usize = match args[0].parse() {
        Ok(n) => n,
        Err(_) => return format!("error: invalid space number '{}'", args[0]),
    };

    let mut s = state.lock().unwrap();
    let cid = s.connection_id;

    // Get list of all spaces
    let spaces = s.skylight.get_managed_display_spaces(cid);
    if space_idx == 0 || space_idx > spaces.len() {
        return format!("error: space {} does not exist (have {})", space_idx, spaces.len());
    }

    let target_space = spaces[space_idx - 1];
    if let Some(focused_id) = s.tracker.get_focused().map(|w| w.id) {
        // Use SkyLight to move the window to the target space
        s.skylight
            .move_windows_to_space(cid, &[focused_id], target_space);

        // Update our internal tracking
        if let Some(w) = s.tracker.get_window_mut(focused_id) {
            let old_space = w.space_id;
            w.space_id = target_space;

            // Remove from old space layout, add to new
            if let Some(layout) = s.layouts.get_mut(&old_space) {
                layout.remove_window(focused_id);
            }
            s.layout_for_space(target_space).add_window(focused_id);

            // Retile both spaces
            s.retile_space(old_space);
            s.retile_space(target_space);
        }

        return format!("ok: moved window {} to space {}", focused_id, space_idx);
    }
    "error: no focused window".to_string()
}

fn handle_switch_space(args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    if args.is_empty() {
        return "error: space requires a space number".to_string();
    }
    let space_idx: usize = match args[0].parse() {
        Ok(n) => n,
        Err(_) => return format!("error: invalid space number '{}'", args[0]),
    };

    let s = state.lock().unwrap();
    let cid = s.connection_id;
    let spaces = s.skylight.get_managed_display_spaces(cid);
    if space_idx == 0 || space_idx > spaces.len() {
        return format!("error: space {} does not exist", space_idx);
    }

    let target_space = spaces[space_idx - 1];
    s.skylight.switch_to_space(cid, target_space);
    format!("ok: switched to space {}", space_idx)
}

fn handle_layout(args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    if args.is_empty() {
        return "error: layout requires a mode (bsp|monocle|master-stack)".to_string();
    }
    let mode = match args[0] {
        "bsp" => LayoutMode::Bsp,
        "monocle" => LayoutMode::Monocle,
        "master-stack" | "master_stack" => LayoutMode::MasterStack {
            master_ratio: 0.55,
        },
        _ => return format!("error: unknown layout mode '{}'", args[0]),
    };

    let mut s = state.lock().unwrap();
    let active_space = s.skylight.get_active_space(s.connection_id);
    s.layout_for_space(active_space).set_mode(mode);
    s.retile_space(active_space);
    format!("ok: layout set to {:?}", mode)
}

fn handle_toggle(args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    if args.is_empty() {
        return "error: toggle requires a target (float|fullscreen)".to_string();
    }
    match args[0] {
        "float" => {
            let mut s = state.lock().unwrap();
            if let Some(focused_id) = s.tracker.get_focused().map(|w| w.id) {
                s.tracker.toggle_floating(focused_id);
                let active_space = s.skylight.get_active_space(s.connection_id);
                s.retile_space(active_space);
                return format!("ok: toggled float for window {}", focused_id);
            }
            "error: no focused window".to_string()
        }
        "fullscreen" => {
            // TODO: implement native fullscreen toggle via AX API
            "ok: fullscreen toggle (not yet implemented)".to_string()
        }
        _ => format!("error: unknown toggle target '{}'", args[0]),
    }
}

fn handle_rotate(_args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    let mut s = state.lock().unwrap();
    let active_space = s.skylight.get_active_space(s.connection_id);
    s.layout_for_space(active_space).rotate_tree();
    s.retile_space(active_space);
    "ok: tree rotated".to_string()
}

fn handle_equalize(_args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    let mut s = state.lock().unwrap();
    let active_space = s.skylight.get_active_space(s.connection_id);
    s.layout_for_space(active_space).equalize_tree();
    s.retile_space(active_space);
    "ok: tree equalized".to_string()
}

fn handle_resize(args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    if args.len() < 2 {
        return "error: resize requires <direction> <delta>".to_string();
    }
    let direction = match args[0] {
        "left" | "west" => SplitDirection::Horizontal,
        "right" | "east" => SplitDirection::Horizontal,
        "up" | "north" => SplitDirection::Vertical,
        "down" | "south" => SplitDirection::Vertical,
        _ => return format!("error: invalid direction '{}'", args[0]),
    };
    let delta: f64 = match args[1].parse() {
        Ok(d) => {
            // Negative delta for left/up, positive for right/down
            match args[0] {
                "left" | "west" | "up" | "north" => -d,
                _ => d,
            }
        }
        Err(_) => return format!("error: invalid delta '{}'", args[1]),
    };

    let mut s = state.lock().unwrap();
    let active_space = s.skylight.get_active_space(s.connection_id);
    if let Some(focused_id) = s.tracker.get_focused().map(|w| w.id) {
        s.layout_for_space(active_space)
            .resize_window(focused_id, direction, delta);
        s.retile_space(active_space);
        return format!("ok: resized window {}", focused_id);
    }
    "error: no focused window".to_string()
}

fn handle_retile(state: &Arc<Mutex<Mosaic>>) -> String {
    let mut s = state.lock().unwrap();
    let active_space = s.skylight.get_active_space(s.connection_id);
    s.tracker.refresh();
    s.retile_space(active_space);
    "ok: retiled".to_string()
}

fn handle_query(args: &[&str], state: &Arc<Mutex<Mosaic>>) -> String {
    if args.is_empty() {
        return "error: query requires a target (windows|spaces|focused)".to_string();
    }
    let s = state.lock().unwrap();
    match args[0] {
        "windows" => {
            let windows: Vec<String> = s
                .tracker
                .all_windows()
                .iter()
                .map(|w| {
                    format!(
                        "{{\"id\":{},\"app\":\"{}\",\"title\":\"{}\",\"space\":{},\"floating\":{}}}",
                        w.id, w.owner_name, w.title, w.space_id, w.is_floating
                    )
                })
                .collect();
            format!("[{}]", windows.join(","))
        }
        "spaces" => {
            let cid = s.connection_id;
            let spaces = s.skylight.get_managed_display_spaces(cid);
            let active = s.skylight.get_active_space(cid);
            let space_strs: Vec<String> = spaces
                .iter()
                .enumerate()
                .map(|(i, &sid)| {
                    format!(
                        "{{\"index\":{},\"id\":{},\"active\":{}}}",
                        i + 1,
                        sid,
                        sid == active
                    )
                })
                .collect();
            format!("[{}]", space_strs.join(","))
        }
        "focused" => {
            if let Some(w) = s.tracker.get_focused() {
                format!(
                    "{{\"id\":{},\"app\":\"{}\",\"title\":\"{}\"}}",
                    w.id, w.owner_name, w.title
                )
            } else {
                "null".to_string()
            }
        }
        _ => format!("error: unknown query target '{}'", args[0]),
    }
}

fn parse_direction(s: &str) -> Option<Direction> {
    match s {
        "north" | "up" => Some(Direction::North),
        "south" | "down" => Some(Direction::South),
        "east" | "right" => Some(Direction::East),
        "west" | "left" => Some(Direction::West),
        _ => None,
    }
}
