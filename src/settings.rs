use std::{fs, path::PathBuf};

use crate::runtime;

/// Persistent application settings, serialised to a JSON file in the user's config directory.
#[derive(Clone, Debug, PartialEq)]
pub struct AppSettings {
    pub default_output_folder: Option<PathBuf>,
    pub photo_output_folder: Option<PathBuf>,
    pub audio_output_folder: Option<PathBuf>,
    pub video_output_folder: Option<PathBuf>,
    pub document_output_folder: Option<PathBuf>,
    pub use_hardware_acceleration: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_output_folder: None,
            photo_output_folder: None,
            audio_output_folder: None,
            video_output_folder: None,
            document_output_folder: None,
            use_hardware_acceleration: true,
        }
    }
}

impl AppSettings {
    fn config_path() -> Option<PathBuf> {
        runtime::config_dir().map(|dir| dir.join("settings.json"))
    }

    fn legacy_config_path() -> Option<PathBuf> {
        runtime::legacy_config_dir().map(|dir| dir.join("settings.json"))
    }

    /// Load settings from disk. Returns `Default` on any error.
    pub fn load() -> Self {
        // Prefer the rebranded config path, but keep reading the legacy one
        // during the transition so existing settings still load.
        let path = match Self::config_path()
            .filter(|path| path.exists())
            .or_else(|| Self::legacy_config_path().filter(|path| path.exists()))
            .or_else(Self::config_path)
        {
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

    pub fn preferred_photo_output_folder(&self) -> Option<PathBuf> {
        self.photo_output_folder
            .clone()
            .or_else(|| self.default_output_folder.clone())
    }

    pub fn preferred_video_output_folder(&self) -> Option<PathBuf> {
        self.video_output_folder
            .clone()
            .or_else(|| self.default_output_folder.clone())
    }

    /// Returns the document output override, or the shared default output folder when unset.
    pub fn preferred_document_output_folder(&self) -> Option<PathBuf> {
        self.document_output_folder
            .clone()
            .or_else(|| self.default_output_folder.clone())
    }

    /// Returns the audio output override, or the shared default output folder when unset.
    pub fn preferred_audio_output_folder(&self) -> Option<PathBuf> {
        self.audio_output_folder
            .clone()
            .or_else(|| self.default_output_folder.clone())
    }

    fn to_json(&self) -> Result<String, ()> {
        Ok(format!(
            "{{\"default_output_folder\":{},\"photo_output_folder\":{},\"audio_output_folder\":{},\"video_output_folder\":{},\"document_output_folder\":{},\"use_hardware_acceleration\":{}}}",
            path_to_json_value(self.default_output_folder.as_ref()),
            path_to_json_value(self.photo_output_folder.as_ref()),
            path_to_json_value(self.audio_output_folder.as_ref()),
            path_to_json_value(self.video_output_folder.as_ref()),
            path_to_json_value(self.document_output_folder.as_ref()),
            if self.use_hardware_acceleration {
                "true"
            } else {
                "false"
            },
        ))
    }

    fn from_json(text: &str) -> Option<Self> {
        // Minimal hand-rolled JSON parser to avoid pulling in serde.
        let text = text.trim();
        if !text.starts_with('{') || !text.ends_with('}') {
            return None;
        }

        Some(Self {
            default_output_folder: parse_optional_path_field(text, "default_output_folder").ok()?,
            photo_output_folder: parse_optional_path_field(text, "photo_output_folder").ok()?,
            audio_output_folder: parse_optional_path_field(text, "audio_output_folder").ok()?,
            video_output_folder: parse_optional_path_field(text, "video_output_folder").ok()?,
            document_output_folder: parse_optional_path_field(text, "document_output_folder")
                .ok()?,
            use_hardware_acceleration: parse_optional_bool_field(text, "use_hardware_acceleration")
                .ok()?
                .unwrap_or(true),
        })
    }
}

fn path_to_json_value(path: Option<&PathBuf>) -> String {
    let value = path.and_then(|value| value.to_str()).unwrap_or("");
    if value.is_empty() {
        "null".to_owned()
    } else {
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    }
}

fn parse_optional_path_field(text: &str, field: &str) -> Result<Option<PathBuf>, ()> {
    let key = format!("\"{field}\"");
    let Some(key_pos) = text.find(&key) else {
        return Ok(None);
    };

    let after_key = &text[key_pos + key.len()..];
    let Some(colon_pos) = after_key.find(':') else {
        return Err(());
    };
    let after_colon = after_key[colon_pos + 1..].trim_start();

    if after_colon.starts_with("null") {
        return Ok(None);
    }

    let Some(after_quote) = after_colon.strip_prefix('"') else {
        return Err(());
    };

    let value = parse_json_string(after_quote).ok_or(())?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(value)))
    }
}

fn parse_json_string(input: &str) -> Option<String> {
    let mut escaped = false;
    let mut value = String::new();

    for ch in input.chars() {
        if escaped {
            value.push(match ch {
                '\\' => '\\',
                '"' => '"',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }

    None
}

fn parse_optional_bool_field(text: &str, field: &str) -> Result<Option<bool>, ()> {
    let key = format!("\"{field}\"");
    let Some(key_pos) = text.find(&key) else {
        return Ok(None);
    };

    let after_key = &text[key_pos + key.len()..];
    let Some(colon_pos) = after_key.find(':') else {
        return Err(());
    };
    let after_colon = after_key[colon_pos + 1..].trim_start();

    if after_colon.starts_with("true") {
        Ok(Some(true))
    } else if after_colon.starts_with("false") {
        Ok(Some(false))
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppSettings;
    use std::path::PathBuf;

    #[test]
    fn parses_legacy_settings_file() {
        let settings =
            AppSettings::from_json(r#"{"default_output_folder":"C:\\Exports"}"#).unwrap();

        assert_eq!(
            settings.default_output_folder,
            Some(PathBuf::from(r"C:\Exports"))
        );
        assert_eq!(settings.photo_output_folder, None);
        assert_eq!(settings.audio_output_folder, None);
        assert_eq!(settings.video_output_folder, None);
        assert_eq!(settings.document_output_folder, None);
        assert!(settings.use_hardware_acceleration);
    }

    #[test]
    fn round_trips_all_output_folders() {
        let settings = AppSettings {
            default_output_folder: Some(PathBuf::from(r"C:\Exports")),
            photo_output_folder: Some(PathBuf::from(r"D:\Photos")),
            audio_output_folder: Some(PathBuf::from(r"F:\Audio")),
            video_output_folder: Some(PathBuf::from(r"E:\Videos")),
            document_output_folder: Some(PathBuf::from(r"G:\Documents")),
            use_hardware_acceleration: false,
        };

        let encoded = settings.to_json().unwrap();
        let decoded = AppSettings::from_json(&encoded).unwrap();

        assert_eq!(decoded, settings);
    }
}
