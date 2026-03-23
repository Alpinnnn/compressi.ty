use std::{fs, path::PathBuf};

use crate::runtime;

/// Persistent application settings, serialised to a JSON file in the user's config directory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppSettings {
    pub default_output_folder: Option<PathBuf>,
}

impl AppSettings {
    fn config_path() -> Option<PathBuf> {
        runtime::config_dir().map(|dir| dir.join("settings.json"))
    }

    /// Load settings from disk. Returns `Default` on any error.
    pub fn load() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => return Self::default(),
        };
        Self::from_json(&text).unwrap_or_default()
    }

    /// Persist settings to disk. Silently ignores errors.
    pub fn save(&self) {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return,
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = self.to_json() {
            let _ = fs::write(&path, json);
        }
    }

    fn to_json(&self) -> Result<String, ()> {
        let folder = self
            .default_output_folder
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        let value = if folder.is_empty() {
            "null".to_owned()
        } else {
            // Escape backslashes and quotes for a JSON string value
            let escaped = folder.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        };
        Ok(format!("{{\"default_output_folder\":{value}}}"))
    }

    fn from_json(text: &str) -> Option<Self> {
        // Minimal hand-rolled JSON parser — avoids pulling in serde.
        let text = text.trim();
        // Extract value for "default_output_folder"
        let key = "\"default_output_folder\"";
        let key_pos = text.find(key)?;
        let after_colon = text[key_pos + key.len()..].trim_start_matches([' ', ':', '\t']);
        if after_colon.starts_with("null") {
            return Some(Self {
                default_output_folder: None,
            });
        }
        if after_colon.starts_with('"') {
            let inner = &after_colon[1..];
            let end = inner.find('"')?;
            let raw = &inner[..end];
            // Unescape basic sequences
            let unescaped = raw.replace("\\\\", "\\").replace("\\\"", "\"");
            if unescaped.is_empty() {
                return Some(Self {
                    default_output_folder: None,
                });
            }
            return Some(Self {
                default_output_folder: Some(PathBuf::from(unescaped)),
            });
        }
        None
    }
}
