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
mod window;
mod layout;
mod msg;
mod hotkeys;
mod ipc;
mod skylight;
mod features;
mod sa;

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
    /// Store observers so they don't drop
    pub observers: Vec<accessibility::AXObserver>,
    
    /// Windows banished to the hidden scratchpad
    pub scratchpad: Vec<u32>,
    /// Windows that should float over the tiling layout
    pub floating_windows: std::collections::HashSet<u32>,
    /// Marks mapping keys to window IDs
    pub marks: std::collections::HashMap<char, u32>,
    /// The Scripting Addition Mach IPC client (if injected into Dock)
    pub sa_client: Option<sa::ScriptingAddition>,
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
            observers: Vec::new(),
            scratchpad: Vec::new(),
            floating_windows: std::collections::HashSet::new(),
            marks: std::collections::HashMap::new(),
            sa_client: sa::ScriptingAddition::new(),
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
            if self.floating_windows.contains(&wid) { continue; } // Don't tile floaters
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



extern "C" fn ax_observer_callback(
    _observer: accessibility::AXObserverRef,
    _element: accessibility::AXUIElementRef,
    _notification: core_foundation::string::CFStringRef,
    refcon: *mut std::os::raw::c_void,
) {
    if refcon.is_null() { return; }
    let state_mutex = unsafe { &*(refcon as *const Mutex<Mosaic>) };
    
    // In a real app we'd decode `notification` and do fine-grained updates.
    // For now, any event triggers a full refresh and retile.
    if let Ok(mut s) = state_mutex.lock() {
        s.tracker.refresh();
        let cid = s.connection_id;
        let active_space = s.skylight.get_active_space(cid);
        s.retile_space(active_space);
    }
}

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
            warn!("Failed to load config ({:?}), generating default configuration at {:?}", e, config_path);
            let _ = MosaicConfig::write_default(&config_path);
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
            s.tracker.managed_windows().len(),
            active_space
        );
        s.retile_space(active_space);
        
        // Wire up AXObserver for each unique PID
        let mut pids = std::collections::HashSet::new();
        for w in s.tracker.all_windows() {
            pids.insert(w.owner_pid);
        }
        
        let state_ptr = Arc::as_ptr(&state) as *mut std::os::raw::c_void;
        for pid in pids {
            if let Ok(app_el) = accessibility::AXElement::application(pid) {
                if let Ok(obs) = accessibility::AXObserver::new(pid, ax_observer_callback) {
                    let _ = obs.add_notification(&app_el, "AXWindowCreated", state_ptr);
                    let _ = obs.add_notification(&app_el, "AXUIElementDestroyed", state_ptr);
                    let _ = obs.add_notification(&app_el, "AXFocusedWindowChanged", state_ptr);
                    obs.attach_to_run_loop();
                    s.observers.push(obs);
                }
            }
        }
        info!("Registered AXObservers for {} processes", s.observers.len());
    }
    
    // Start native hotkey listener
    hotkeys::start_hotkey_listener(Arc::clone(&state));

    info!("Mosaic is running. Send commands via `mosaic-msg`.");

    // Set up signal handler for graceful shutdown
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    signal_hook::flag::register(signal_hook::consts::SIGINT, r.clone()).ok();
    signal_hook::flag::register(signal_hook::consts::SIGTERM, r.clone()).ok();

    // Space polling background thread
    let poll_state = Arc::clone(&state);
    let poll_running = r.clone();
    std::thread::spawn(move || {
        let mut last_space = 0;
        while poll_running.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if let Ok(mut s) = poll_state.lock() {
                let cid = s.connection_id;
                let active_space = s.skylight.get_active_space(cid);
                if active_space != last_space {
                    if last_space != 0 {
                        info!("Space changed: {} -> {}", last_space, active_space);
                        s.tracker.refresh();
                        s.retile_space(active_space);
                    }
                    last_space = active_space;
                }
            }
        }
    });

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
