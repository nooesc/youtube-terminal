use crate::player::PlaybackQuality;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mpv_geometry: String,
    pub mpv_ontop: bool,
    pub default_playback_quality: PlaybackQuality,
    pub mpv_hwdec: String,
    pub mpv_cache_secs: u32,
    pub mpv_cache_pause_wait: f64,
    pub mpv_force_seekable: bool,
    pub mpv_demuxer_max_bytes: String,
    pub mpv_demuxer_max_back_bytes: String,
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
            default_playback_quality: PlaybackQuality::P1080,
            mpv_hwdec: "auto-safe".to_string(),
            mpv_cache_secs: 45,
            mpv_cache_pause_wait: 1.5,
            mpv_force_seekable: true,
            mpv_demuxer_max_bytes: "128MiB".to_string(),
            mpv_demuxer_max_back_bytes: "64MiB".to_string(),
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
        self.session_dir().join("cookies.txt")
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

    pub fn session_dir(&self) -> PathBuf {
        self.data_dir.join("session")
    }

    pub fn player_socket_path(&self) -> PathBuf {
        self.session_dir().join("mpv.sock")
    }

    pub fn session_state_path(&self) -> PathBuf {
        self.session_dir().join("app-state.json")
    }

    pub fn mpv_log_path(&self) -> PathBuf {
        self.cache_dir.join("logs").join("mpv.log")
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::player::PlaybackQuality;

    #[test]
    fn parses_legacy_config_with_new_defaults() {
        let config: Config = toml::from_str(
            r#"
            mpv_geometry = "1280x720"
            mpv_ontop = false
            data_dir = "/tmp/youtube-terminal"
            cache_dir = "/tmp/youtube-terminal/cache"
            "#,
        )
        .expect("legacy config should still parse");

        assert_eq!(config.mpv_geometry, "1280x720");
        assert!(!config.mpv_ontop);
        assert_eq!(config.default_playback_quality, PlaybackQuality::P1080);
        assert_eq!(config.mpv_hwdec, "auto-safe");
        assert_eq!(config.mpv_cache_secs, 45);
        assert_eq!(config.mpv_cache_pause_wait, 1.5);
        assert!(config.mpv_force_seekable);
        assert_eq!(config.mpv_demuxer_max_bytes, "128MiB");
        assert_eq!(config.mpv_demuxer_max_back_bytes, "64MiB");
    }

    #[test]
    fn parses_default_playback_quality() {
        let config: Config = toml::from_str(
            r#"
            default_playback_quality = "720p"
            data_dir = "/tmp/youtube-terminal"
            cache_dir = "/tmp/youtube-terminal/cache"
            "#,
        )
        .expect("quality config should parse");

        assert_eq!(config.default_playback_quality, PlaybackQuality::P720);
    }
}
