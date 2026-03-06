use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub mpv_geometry: String,
    pub mpv_ontop: bool,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let base = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("youtube-terminal");
        let data_dir = base.clone();
        let cache_dir = base.join("cache");
        Self {
            mpv_geometry: "800x450+50%+50%".to_string(),
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
