use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Config {
    pub cookie_file: Option<PathBuf>,
    pub mpv_geometry: String,
    pub mpv_ontop: bool,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("youtube-terminal");
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("~/.cache"))
            .join("youtube-terminal");
        Self {
            cookie_file: None,
            mpv_geometry: "400x225+0+0".to_string(),
            mpv_ontop: true,
            data_dir,
            cache_dir,
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("youtube-terminal")
            .join("config.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn cookie_path(&self) -> PathBuf {
        self.data_dir.join("session").join("cookies.txt")
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("youtube-terminal.db")
    }

    pub fn thumbnail_dir(&self) -> PathBuf {
        self.cache_dir.join("thumbs")
    }

    pub fn rustypipe_storage_dir(&self) -> PathBuf {
        self.data_dir.join("rustypipe")
    }
}
