//! # Mosaic - macOS Tiling Window Manager
//!
//! A fast, keyboard-driven tiling window manager for macOS that integrates with
//! native macOS Spaces and uses private SkyLight APIs for deep system integration.
//!
//! ## Architecture
//!
//! Mosaic runs as a headless daemon with four core components:
//!
//! 1. **Observer** - Watches for macOS window events via the Accessibility API
//! 2. **State Manager (WindowTracker)** - Maintains an in-memory model of all windows
//! 3. **Layout Engine** - Computes window positions using BSP/Monocle/MasterStack algorithms
//! 4. **IPC Server** - Listens on a Unix socket for commands from `mosaic-msg` or hotkey daemons

mod accessibility;
mod config;
mod ipc;
mod layout;
mod skylight;
mod window;

use log::{error, info, warn};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::MosaicConfig;
use crate::ipc::IpcServer;
use crate::layout::{LayoutEngine, LayoutMode, Rect};
use crate::skylight::SkyLight;
use crate::window::WindowTracker;

/// The socket path for IPC communication
const SOCKET_PATH: &str = "/tmp/mosaic.sock";

/// Central application state shared across threads
pub struct Mosaic {
    /// The configuration loaded from disk
    pub config: MosaicConfig,
    /// Tracks all windows on the system
    pub tracker: WindowTracker,
    /// Per-space layout engines (space_id -> LayoutEngine)
    pub layouts: std::collections::HashMap<u64, LayoutEngine>,
    /// SkyLight private API bindings
    pub skylight: SkyLight,
    /// The connection ID to the macOS Window Server
    pub connection_id: i32,
}

impl Mosaic {
    /// Create a new Mosaic instance, loading config and initializing subsystems.
    pub fn new(config: MosaicConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Load private SkyLight framework
        let skylight = SkyLight::new().map_err(|e| {
            error!("Failed to load SkyLight framework: {:?}", e);
            e
        })?;

        // Get connection to the macOS Window Server
        let connection_id = skylight.main_connection_id();
        info!("Connected to macOS Window Server (cid={})", connection_id);

        // Initialize window tracker with rules from config
        let mut tracker = WindowTracker::new();
        for rule in &config.rules {
            tracker.add_rule(crate::window::WindowRule {
                app_name: rule.app_name.clone(),
                title_contains: rule.title_contains.clone(),
                action: match rule.action.as_str() {
                    "float" => crate::window::RuleAction::Float,
                    "ignore" => crate::window::RuleAction::Ignore,
                    _ => crate::window::RuleAction::Float,
                },
            });
        }

        Ok(Self {
            config,
            tracker,
            layouts: std::collections::HashMap::new(),
            skylight,
            connection_id,
        })
    }

    /// Get or create a layout engine for the given space.
    pub fn layout_for_space(&mut self, space_id: u64) -> &mut LayoutEngine {
        let config = &self.config;
        self.layouts.entry(space_id).or_insert_with(|| {
            LayoutEngine::new(
                LayoutMode::Bsp,
                config.gaps.inner as f64,
                config.gaps.outer as f64,
            )
        })
    }

    /// Perform a full retile of the given space.
    ///
    /// 1. Query which windows are on this space
    /// 2. Compute the layout
    /// 3. Move windows to their computed positions via Accessibility API
    pub fn retile_space(&mut self, space_id: u64) {
        let managed: Vec<u32> = self
            .tracker
            .windows_on_space(space_id)
            .iter()
            .filter(|w| !w.is_floating && !w.is_minimized)
            .map(|w| w.id)
            .collect();

        if managed.is_empty() {
            return;
        }

        // Ensure all managed windows are in the layout engine
        let layout = self.layout_for_space(space_id);
        for &wid in &managed {
            if !layout.has_window(wid) {
                layout.add_window(wid);
            }
        }

        // TODO: Get actual screen rect from NSScreen, accounting for menu bar and dock
        let screen = Rect {
            x: 0.0,
            y: 25.0, // menu bar height
            width: 1920.0,
            height: 1055.0,
        };

        let rects = layout.compute_layout(screen);

        for (wid, rect) in rects {
            if let Some(window) = self.tracker.get_window(wid) {
                // Use Accessibility API to move + resize the window
                if let Ok(app_element) =
                    accessibility::AXElement::application(window.owner_pid)
                {
                    if let Ok(windows) = app_element.get_windows() {
                        for ax_win in windows {
                            // Match by window title or position heuristic
                            if let Ok(title) = ax_win.get_title() {
                                if title == window.title {
                                    let _ = ax_win.set_position(rect.x, rect.y);
                                    let _ = ax_win.set_size(rect.width, rect.height);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

mod skylight;
pub mod layout;
pub mod accessibility;

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("Mosaic v{} starting...", env!("CARGO_PKG_VERSION"));

    // Check accessibility permissions
    if !accessibility::is_trusted() {
        warn!("Mosaic does not have Accessibility permissions!");
        warn!("Please grant access in System Settings > Privacy & Security > Accessibility");
        accessibility::request_trust();
        // Continue anyway — some features will work, others won't
    }

    // Load configuration
    let config_path = dirs_config_path();
    let config = match MosaicConfig::load(&config_path) {
        Ok(cfg) => {
            info!("Loaded config from {:?}", config_path);
            cfg
        }
        Err(e) => {
            warn!("Failed to load config ({:?}), using defaults", e);
            MosaicConfig::default()
        }
    };

    // Initialize the core state
    let mosaic = match Mosaic::new(config) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to initialize Mosaic: {}", e);
            std::process::exit(1);
        }
    };

    let state = Arc::new(Mutex::new(mosaic));

    // Start IPC server in a background thread
    let ipc_state = Arc::clone(&state);
    std::thread::spawn(move || {
        info!("Starting IPC server on {}", SOCKET_PATH);
        if let Err(e) = IpcServer::new(SOCKET_PATH).and_then(|server| server.run(ipc_state)) {
            error!("IPC server error: {}", e);
        }
    });

    // Initial window discovery
    {
        let mut s = state.lock().unwrap();
        s.tracker.refresh();
        let active_space = s.skylight.get_active_space(s.connection_id);
        info!(
            "Discovered {} windows, active space: {}",
            s.tracker.window_count(),
            active_space
        );
        s.retile_space(active_space);
    }

    info!("Mosaic is running. Send commands via `mosaic-msg`.");

    // Set up signal handler for graceful shutdown
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    signal_hook::flag::register(signal_hook::consts::SIGINT, r.clone()).ok();
    signal_hook::flag::register(signal_hook::consts::SIGTERM, r.clone()).ok();

    // Main run loop — process macOS events
    // The CFRunLoop is needed for Accessibility API observers to fire callbacks
    while running.load(std::sync::atomic::Ordering::Relaxed) {
        // Run the macOS run loop for a short interval to process events
        // This allows AXObserver notifications and NSWorkspace notifications to fire
        unsafe {
            core_foundation::runloop::CFRunLoopRunInMode(
                core_foundation::runloop::kCFRunLoopDefaultMode,
                0.1, // 100ms intervals
                false as u8,
            );
        }
    }

    // Cleanup
    info!("Mosaic shutting down...");
    let _ = std::fs::remove_file(SOCKET_PATH);
}

/// Get the config directory path: ~/.config/mosaic/config.toml
fn dirs_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("mosaic")
        .join("config.toml")
}
