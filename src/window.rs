//! Window tracker and state management for the mosaic tiling window manager.
//!
//! This module bridges the macOS CGWindow API and the Accessibility API to maintain
//! a live picture of every window on screen. It owns the canonical list of
//! [`TrackedWindow`]s and decides which ones are "managed" (actively tiled) versus
//! floating or ignored.

use std::collections::{HashMap, HashSet};

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};

use crate::layout::Rect;

// ---------------------------------------------------------------------------
// CGWindow FFI
// ---------------------------------------------------------------------------

/// Opaque window identifier used by Core Graphics.
pub type CGWindowID = u32;

/// Bitmask options for [`CGWindowListCopyWindowInfo`].
pub type CGWindowListOption = u32;

/// Include only windows that are currently on-screen.
pub const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: CGWindowListOption = 1 << 0;

/// Exclude desktop elements (wallpaper, desktop icons).
pub const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: CGWindowListOption = 1 << 4;

// Dictionary keys returned by CGWindowListCopyWindowInfo.
// These are the CFString values used as keys in each per-window CFDictionary.
const KCGWINDOW_NUMBER: &str = "kCGWindowNumber";
const KCGWINDOW_OWNER_PID: &str = "kCGWindowOwnerPID";
const KCGWINDOW_OWNER_NAME: &str = "kCGWindowOwnerName";
const KCGWINDOW_NAME: &str = "kCGWindowName";
const KCGWINDOW_BOUNDS: &str = "kCGWindowBounds";
const KCGWINDOW_LAYER: &str = "kCGWindowLayer";
const KCGWINDOW_IS_ONSCREEN: &str = "kCGWindowIsOnscreen";

extern "C" {
    /// Returns an array of [`CFDictionary`] values, one per window, describing
    /// every window that matches the given options.
    fn CGWindowListCopyWindowInfo(
        option: CGWindowListOption,
        relative_to_window: CGWindowID,
    ) -> CFArrayRef;
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Represents a single tracked window on the system.
#[derive(Debug, Clone)]
pub struct TrackedWindow {
    /// Core Graphics window identifier.
    pub id: u32,
    /// Process ID of the owning application.
    pub owner_pid: i32,
    /// Human-readable application name (e.g. "Safari").
    pub owner_name: String,
    /// Window title (may be empty for untitled windows).
    pub title: String,
    /// Current position and size of the window.
    pub frame: Rect,
    /// Whether the window is currently minimized to the Dock.
    pub is_minimized: bool,
    /// User has explicitly toggled this window into floating mode.
    pub is_floating: bool,
    /// The macOS Space (virtual desktop) this window lives on.
    pub space_id: u64,
    /// Whether the window is in native macOS fullscreen mode.
    pub is_fullscreen: bool,
}

/// A rule that determines how a window should be handled by the tiler.
#[derive(Debug, Clone)]
pub struct WindowRule {
    /// Match against the owning application's name.
    pub app_name: Option<String>,
    /// Match when the window title contains this substring.
    pub title_contains: Option<String>,
    /// The action to take when the rule matches.
    pub action: RuleAction,
}

/// The action performed when a [`WindowRule`] matches a window.
#[derive(Debug, Clone)]
pub enum RuleAction {
    /// Always float this window (never tile it).
    Float,
    /// Always move this window to the given Space.
    AssignToSpace(u64),
    /// Completely ignore this window – do not manage it at all.
    Ignore,
}

// ---------------------------------------------------------------------------
// WindowTracker
// ---------------------------------------------------------------------------

/// The central state manager that tracks every window on the system.
///
/// `WindowTracker` maintains a map of all known windows, decides which ones
/// are managed (tiled) versus floating or ignored, and applies user-defined
/// rules.
pub struct WindowTracker {
    /// All tracked windows keyed by their CGWindowID.
    windows: HashMap<u32, TrackedWindow>,
    /// The set of window IDs that we are actively tiling.
    managed_windows: HashSet<u32>,
    /// User-defined rules evaluated in order.
    rules: Vec<WindowRule>,
    /// The CGWindowID of the currently focused window, if any.
    focused_window: Option<u32>,
    /// Application names that should never be managed.
    blacklisted_apps: HashSet<String>,
}

impl WindowTracker {
    // -- Default blacklisted applications ----------------------------------

    /// Applications that mosaic should never attempt to manage.
    const DEFAULT_BLACKLISTED_APPS: &[&str] = &[
        "Notification Centre",
        "Control Centre",
        "Dock",
        "WindowManager",
        "SystemUIServer",
    ];

    /// Creates a new, empty `WindowTracker` with the default blacklist.
    pub fn new() -> Self {
        let blacklisted_apps = Self::DEFAULT_BLACKLISTED_APPS
            .iter()
            .map(|s| (*s).to_owned())
            .collect();

        Self {
            windows: HashMap::new(),
            managed_windows: HashSet::new(),
            rules: Vec::new(),
            focused_window: None,
            blacklisted_apps,
        }
    }

    // -- Discovery ---------------------------------------------------------

    /// Queries Core Graphics for all on-screen windows and returns them as a
    /// list of [`TrackedWindow`] values.
    ///
    /// Only windows on layer 0 (standard application windows) are included;
    /// menus, the Dock, overlays, and other system chrome are filtered out.
    pub fn discover_windows(&self) -> Vec<TrackedWindow> {
        let info_array: CFArray<CFType> = unsafe {
            let raw = CGWindowListCopyWindowInfo(
                K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY
                    | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS,
                0, // kCGNullWindowID – relative to nothing
            );
            if raw.is_null() {
                return Vec::new();
            }
            CFArray::wrap_under_create_rule(raw)
        };

        let mut result = Vec::new();

        for i in 0..info_array.len() {
            // Each element is a CFDictionary.
            let entry: CFType = info_array.get(i).unwrap().clone();
            // SAFETY: CGWindowListCopyWindowInfo guarantees CFDictionary values.
            let dict: CFDictionary<CFString, CFType> = unsafe {
                CFDictionary::wrap_under_get_rule(
                    entry.as_CFTypeRef() as *const _,
                )
            };

            if let Some(window) = Self::parse_window_dict(&dict) {
                result.push(window);
            }
        }

        result
    }

    /// Refreshes internal state by re-discovering all windows.
    ///
    /// New windows are added, windows that have disappeared are removed, and
    /// existing windows have their metadata (title, frame, etc.) updated.
    pub fn refresh(&mut self) {
        let discovered = self.discover_windows();
        let mut seen_ids: HashSet<u32> = HashSet::new();

        for window in discovered {
            seen_ids.insert(window.id);

            if self.windows.contains_key(&window.id) {
                // Update the existing record.
                if let Some(existing) = self.windows.get_mut(&window.id) {
                    existing.title = window.title;
                    existing.frame = window.frame;
                    existing.owner_name = window.owner_name;
                    // Preserve user-toggled flags (is_floating, etc.).
                }
            } else {
                // Newly appeared window.
                let id = window.id;
                self.add_window(window);
                self.apply_rules(id);
            }
        }

        // Remove windows that are no longer present.
        let stale_ids: Vec<u32> = self
            .windows
            .keys()
            .filter(|id| !seen_ids.contains(id))
            .copied()
            .collect();

        for id in stale_ids {
            self.remove_window(id);
        }
    }

    // -- Window management -------------------------------------------------

    /// Begins tracking a window.
    ///
    /// If the window passes [`Self::is_manageable`] and its owning app is not
    /// blacklisted, it is also added to the managed (tiled) set.
    pub fn add_window(&mut self, window: TrackedWindow) {
        let id = window.id;
        let manageable =
            Self::is_manageable(&window) && !self.blacklisted_apps.contains(&window.owner_name);

        self.windows.insert(id, window);

        if manageable {
            self.managed_windows.insert(id);
        }
    }

    /// Stops tracking a window entirely.
    pub fn remove_window(&mut self, id: u32) {
        self.windows.remove(&id);
        self.managed_windows.remove(&id);
        if self.focused_window == Some(id) {
            self.focused_window = None;
        }
    }

    /// Returns a reference to the tracked window with the given ID, if any.
    pub fn get_window(&self, id: u32) -> Option<&TrackedWindow> {
        self.windows.get(&id)
    }

    /// Returns a mutable reference to the tracked window with the given ID.
    pub fn get_window_mut(&mut self, id: u32) -> Option<&mut TrackedWindow> {
        self.windows.get_mut(&id)
    }

    /// Returns all windows that are currently being actively tiled.
    pub fn managed_windows(&self) -> Vec<&TrackedWindow> {
        self.managed_windows
            .iter()
            .filter_map(|id| self.windows.get(id))
            .collect()
    }

    /// Returns all tracked windows that belong to the given macOS Space.
    pub fn windows_on_space(&self, space_id: u64) -> Vec<&TrackedWindow> {
        self.windows
            .values()
            .filter(|w| w.space_id == space_id)
            .collect()
    }

    pub fn all_windows(&self) -> Vec<&TrackedWindow> {
        self.windows.values().collect()
    }

    // -- Focus -------------------------------------------------------------

    /// Sets the currently focused window.
    ///
    /// Does nothing if `id` is not a tracked window.
    pub fn set_focused(&mut self, id: u32) {
        if self.windows.contains_key(&id) {
            self.focused_window = Some(id);
        }
    }

    /// Returns a reference to the currently focused window, if any.
    pub fn get_focused(&self) -> Option<&TrackedWindow> {
        self.focused_window
            .and_then(|id| self.windows.get(&id))
    }

    // -- Floating ----------------------------------------------------------

    /// Toggles a window between floating and tiled states.
    ///
    /// A floating window is rendered at its current position and is not
    /// rearranged by the tiling engine. When toggled back to tiled, the
    /// window is re-added to the managed set.
    pub fn toggle_floating(&mut self, id: u32) {
        if let Some(window) = self.windows.get_mut(&id) {
            window.is_floating = !window.is_floating;

            if window.is_floating {
                self.managed_windows.remove(&id);
            } else if Self::is_manageable(window)
                && !self.blacklisted_apps.contains(&window.owner_name)
            {
                self.managed_windows.insert(id);
            }
        }
    }

    // -- Rules -------------------------------------------------------------

    /// Applies all matching rules to the window with the given ID.
    ///
    /// Rules are evaluated in the order they were added. Every matching rule
    /// is applied (not short-circuited).
    pub fn apply_rules(&mut self, window_id: u32) {
        // Collect matching actions first to avoid borrow conflicts.
        let actions: Vec<RuleAction> = {
            let Some(window) = self.windows.get(&window_id) else {
                return;
            };

            self.rules
                .iter()
                .filter(|rule| Self::rule_matches(rule, window))
                .map(|rule| rule.action.clone())
                .collect()
        };

        for action in actions {
            match action {
                RuleAction::Float => {
                    if let Some(w) = self.windows.get_mut(&window_id) {
                        w.is_floating = true;
                    }
                    self.managed_windows.remove(&window_id);
                }
                RuleAction::AssignToSpace(space) => {
                    if let Some(w) = self.windows.get_mut(&window_id) {
                        w.space_id = space;
                    }
                }
                RuleAction::Ignore => {
                    self.managed_windows.remove(&window_id);
                }
            }
        }
    }

    /// Registers a new window rule.
    pub fn add_rule(&mut self, rule: WindowRule) {
        self.rules.push(rule);
    }

    // -- Manageability -----------------------------------------------------

    /// Returns `true` if the window is a standard application window that
    /// should be considered for tiling.
    ///
    /// Windows that are minimized, fullscreen, or have an empty owner name
    /// are not considered manageable.
    pub fn is_manageable(window: &TrackedWindow) -> bool {
        if window.is_minimized || window.is_fullscreen {
            return false;
        }

        // Windows without an owner name are system artefacts.
        if window.owner_name.is_empty() {
            return false;
        }

        // Ignore extremely small windows (likely invisible helper windows).
        if window.frame.width < 1.0 || window.frame.height < 1.0 {
            return false;
        }

        true
    }

    // -- Internal helpers --------------------------------------------------

    /// Checks whether a [`WindowRule`] matches a given [`TrackedWindow`].
    fn rule_matches(rule: &WindowRule, window: &TrackedWindow) -> bool {
        if let Some(ref app) = rule.app_name {
            if window.owner_name != *app {
                return false;
            }
        }

        if let Some(ref substring) = rule.title_contains {
            if !window.title.contains(substring.as_str()) {
                return false;
            }
        }

        true
    }

    /// Parses a single CGWindow info dictionary into a [`TrackedWindow`].
    ///
    /// Returns `None` if required keys are missing or the window should be
    /// skipped (e.g. layer ≠ 0).
    fn parse_window_dict(dict: &CFDictionary<CFString, CFType>) -> Option<TrackedWindow> {
        // Helper: look up a key and downcast to CFNumber → i64.
        let get_number = |key_str: &str| -> Option<i64> {
            let key = CFString::new(key_str);
            dict.find(&key).and_then(|v| {
                // SAFETY: we trust CG to return the correct type for numeric keys.
                let num: CFNumber =
                    unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as *const _) };
                num.to_i64()
            })
        };

        // Helper: look up a key and downcast to CFString → String.
        let get_string = |key_str: &str| -> Option<String> {
            let key = CFString::new(key_str);
            dict.find(&key).map(|v| {
                let cf_str: CFString =
                    unsafe { CFString::wrap_under_get_rule(v.as_CFTypeRef() as CFStringRef) };
                cf_str.to_string()
            })
        };

        // ---- Required fields ----

        let window_id = get_number(KCGWINDOW_NUMBER)? as u32;
        let owner_pid = get_number(KCGWINDOW_OWNER_PID)? as i32;

        // Layer must be 0 (standard windows). Non-zero layers include
        // menus, screensavers, the Dock, etc.
        let layer = get_number(KCGWINDOW_LAYER).unwrap_or(-1);
        if layer != 0 {
            return None;
        }

        // ---- Optional / defaulted fields ----

        let owner_name = get_string(KCGWINDOW_OWNER_NAME).unwrap_or_default();
        let title = get_string(KCGWINDOW_NAME).unwrap_or_default();

        // Parse window bounds from the nested CFDictionary.
        let frame = Self::parse_bounds(dict).unwrap_or(Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        });

        Some(TrackedWindow {
            id: window_id,
            owner_pid,
            owner_name,
            title,
            frame,
            is_minimized: false,
            is_floating: false,
            space_id: 0,
            is_fullscreen: false,
        })
    }

    /// Extracts a [`Rect`] from the `kCGWindowBounds` sub-dictionary.
    ///
    /// The bounds dictionary has keys "X", "Y", "Width", and "Height", each
    /// mapping to a CFNumber.
    fn parse_bounds(dict: &CFDictionary<CFString, CFType>) -> Option<Rect> {
        let bounds_key = CFString::new(KCGWINDOW_BOUNDS);
        let bounds_val = dict.find(&bounds_key)?;

        let bounds_dict: CFDictionary<CFString, CFType> =
            unsafe { CFDictionary::wrap_under_get_rule(bounds_val.as_CFTypeRef() as *const _) };

        let get = |k: &str| -> Option<f64> {
            let key = CFString::new(k);
            bounds_dict.find(&key).and_then(|v| {
                let num: CFNumber =
                    unsafe { CFNumber::wrap_under_get_rule(v.as_CFTypeRef() as *const _) };
                num.to_f64()
            })
        };

        Some(Rect {
            x: get("X")?,
            y: get("Y")?,
            width: get("Width")?,
            height: get("Height")?,
        })
    }
}

impl Default for WindowTracker {
    fn default() -> Self {
        Self::new()
    }
}
