//! Configuration module for Mosaic.
//!
//! Loads and parses the TOML configuration file from `~/.config/mosaic/config.toml`.
//! Provides sane defaults for all settings.

use serde::Deserialize;
use std::path::Path;

/// Top-level configuration structure.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MosaicConfig {
    /// General settings
    pub general: GeneralConfig,
    /// Gap settings
    pub gaps: GapConfig,
    /// Keybinding definitions
    pub keybindings: Vec<Keybinding>,
    /// Window rules for automatic behavior
    pub rules: Vec<RuleConfig>,
    /// Applications that should never be managed
    pub blacklist: Vec<String>,
}

/// General settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Default layout mode: "bsp", "monocle", or "master-stack"
    pub layout: String,
    /// Whether focus follows the mouse cursor
    pub focus_follows_mouse: bool,
    /// Whether to auto-start tiling on launch
    pub auto_tile: bool,
    /// Master-stack ratio (0.0 to 1.0)
    pub master_ratio: f64,
    /// Whether to animate window movements (false = instant)
    pub animate: bool,
    /// Log level: "trace", "debug", "info", "warn", "error"
    pub log_level: String,
}

/// Gap configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GapConfig {
    /// Pixels between windows
    pub inner: u32,
    /// Pixels between windows and screen edges
    pub outer: u32,
}

/// A keybinding definition.
#[derive(Debug, Clone, Deserialize)]
pub struct Keybinding {
    /// Modifier keys: "cmd", "alt", "ctrl", "shift"
    pub modifiers: Vec<String>,
    /// The key to bind (e.g., "h", "j", "return", "space")
    pub key: String,
    /// The action to execute (e.g., "focus east", "swap west", "toggle float")
    pub action: String,
}

/// A window rule for automatic behavior.
#[derive(Debug, Clone, Deserialize)]
pub struct RuleConfig {
    /// Match by application name (optional)
    pub app_name: Option<String>,
    /// Match by window title substring (optional)
    pub title_contains: Option<String>,
    /// Action: "float", "ignore", "space:<id>"
    pub action: String,
}

impl Default for MosaicConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            gaps: GapConfig::default(),
            keybindings: default_keybindings(),
            rules: default_rules(),
            blacklist: default_blacklist(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            layout: "bsp".to_string(),
            focus_follows_mouse: false,
            auto_tile: true,
            master_ratio: 0.5,
            animate: false,
            log_level: "info".to_string(),
        }
    }
}

impl Default for GapConfig {
    fn default() -> Self {
        Self {
            inner: 8,
            outer: 8,
        }
    }
}

impl MosaicConfig {
    /// Load config from the given path. Returns an error if the file doesn't exist
    /// or cannot be parsed.
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: MosaicConfig = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save the default config to disk if no config file exists.
    pub fn write_default(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let default_toml = include_str!("../config/default.toml");
        std::fs::write(path, default_toml)?;
        Ok(())
    }
}

fn default_keybindings() -> Vec<Keybinding> {
    vec![
        // Focus movement
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "h".into(),
            action: "focus west".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "j".into(),
            action: "focus south".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "k".into(),
            action: "focus north".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "l".into(),
            action: "focus east".into(),
        },
        // Window swapping
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "h".into(),
            action: "swap west".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "j".into(),
            action: "swap south".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "k".into(),
            action: "swap north".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "l".into(),
            action: "swap east".into(),
        },
        // Layout toggles
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "f".into(),
            action: "toggle fullscreen".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "space".into(),
            action: "toggle float".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "e".into(),
            action: "layout bsp".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "m".into(),
            action: "layout monocle".into(),
        },
        // Tree manipulation
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "r".into(),
            action: "rotate tree".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "equal".into(),
            action: "equalize tree".into(),
        },
        // Space management
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "1".into(),
            action: "space 1".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "2".into(),
            action: "space 2".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "3".into(),
            action: "space 3".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "4".into(),
            action: "space 4".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into()],
            key: "5".into(),
            action: "space 5".into(),
        },
        // Move window to space
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "1".into(),
            action: "move-to-space 1".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "2".into(),
            action: "move-to-space 2".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "3".into(),
            action: "move-to-space 3".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "4".into(),
            action: "move-to-space 4".into(),
        },
        Keybinding {
            modifiers: vec!["alt".into(), "shift".into()],
            key: "5".into(),
            action: "move-to-space 5".into(),
        },
    ]
}

fn default_rules() -> Vec<RuleConfig> {
    vec![
        RuleConfig {
            app_name: Some("Calculator".into()),
            title_contains: None,
            action: "float".into(),
        },
        RuleConfig {
            app_name: Some("System Settings".into()),
            title_contains: None,
            action: "float".into(),
        },
        RuleConfig {
            app_name: Some("Finder".into()),
            title_contains: Some("Copy".into()),
            action: "float".into(),
        },
    ]
}

fn default_blacklist() -> Vec<String> {
    vec![
        "Notification Centre".into(),
        "Control Centre".into(),
        "Dock".into(),
        "WindowManager".into(),
        "SystemUIServer".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MosaicConfig::default();
        assert_eq!(config.general.layout, "bsp");
        assert_eq!(config.gaps.inner, 8);
        assert_eq!(config.gaps.outer, 8);
    }

    #[test]
    fn test_rule_action_parsing() {
        // We can't directly parse RuleAction here without a mock TOML, 
        // but we can verify its basic structure defaults.
        let default_rules = default_rules();
        assert!(default_rules.len() > 0);
        assert_eq!(default_rules[0].app_name.as_deref(), Some("Calculator"));
    }
}
