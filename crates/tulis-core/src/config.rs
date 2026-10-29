//! User configuration (`config.toml`). Every field has a default.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub editor: EditorConfig,
    pub ai: AiConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub vim_mode: bool,
    pub font_family: String,
    pub font_size: f32,
}

impl EditorConfig {
    /// The configured font size, clamped to 8–72 (the default 18 if it isn't a finite number).
    pub fn effective_font_size(&self) -> f32 {
        if self.font_size.is_finite() {
            self.font_size.clamp(8.0, 72.0)
        } else {
            18.0
        }
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            vim_mode: false,
            font_family: "Iowan Old Style".into(),
            font_size: 18.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Anthropic,
    #[default]
    Openai,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub provider: ProviderKind,
    pub model: String,
    /// Name of the environment variable that holds the API key.
    pub api_key_env: String,
    /// Required for `openai`; optional override for `anthropic`.
    pub base_url: String,
    pub num_alternates: usize,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: ProviderKind::Openai,
            model: "openrouter/auto".into(),
            api_key_env: "OPENROUTER_API_KEY".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            num_alternates: 3,
        }
    }
}

impl Config {
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("tulis").join("config.toml"))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Load from `path`: a missing file gives defaults; a malformed one gives defaults plus a warning.
    pub fn load(path: Option<&Path>) -> (Self, Option<String>) {
        let Some(path) = path else {
            return (Self::default(), None);
        };
        match std::fs::read_to_string(path) {
            Ok(text) => match Self::parse(&text) {
                Ok(config) => (config, None),
                Err(e) => (Self::default(), Some(format!("config.toml ignored: {e}"))),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(e) => (
                Self::default(),
                Some(format!("config.toml unreadable: {e}")),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_font_size_is_clamped_and_finite() {
        let size = |font_size| {
            EditorConfig {
                font_size,
                ..EditorConfig::default()
            }
            .effective_font_size()
        };
        assert_eq!(size(20.0), 20.0);
        assert_eq!(size(2.0), 8.0);
        assert_eq!(size(-5.0), 8.0);
        assert_eq!(size(500.0), 72.0);
        assert_eq!(size(f32::NAN), 18.0);
        assert_eq!(size(f32::INFINITY), 18.0);
    }

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert!(!c.editor.vim_mode);
        assert_eq!(c.editor.font_family, "Iowan Old Style");
        assert_eq!(c.editor.font_size, 18.0);
        assert_eq!(c.ai.provider, ProviderKind::Openai);
        assert_eq!(c.ai.model, "openrouter/auto");
        assert_eq!(c.ai.api_key_env, "OPENROUTER_API_KEY");
        assert_eq!(c.ai.base_url, "https://openrouter.ai/api/v1");
        assert_eq!(c.ai.num_alternates, 3);
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let c = Config::parse("[editor]\nvim_mode = true\n[ai]\nprovider = \"openai\"\nbase_url = \"http://localhost:11434/v1\"\n").unwrap();
        assert!(c.editor.vim_mode);
        assert_eq!(c.editor.font_size, 18.0);
        assert_eq!(c.ai.provider, ProviderKind::Openai);
        assert_eq!(c.ai.base_url, "http://localhost:11434/v1");
        assert_eq!(c.ai.num_alternates, 3);
    }

    #[test]
    fn unknown_provider_is_an_error() {
        assert!(Config::parse("[ai]\nprovider = \"gemini\"\n").is_err());
    }

    #[test]
    fn load_missing_file_gives_defaults_without_warning() {
        let dir = tempfile::tempdir().unwrap();
        let (c, warning) = Config::load(Some(&dir.path().join("config.toml")));
        assert_eq!(c, Config::default());
        assert!(warning.is_none());
        assert_eq!(Config::load(None), (Config::default(), None));
    }

    #[test]
    fn load_malformed_file_gives_defaults_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[editor\nvim_mode = yes").unwrap();
        let (c, warning) = Config::load(Some(&path));
        assert_eq!(c, Config::default());
        assert!(warning.unwrap().starts_with("config.toml ignored:"));
    }
}
