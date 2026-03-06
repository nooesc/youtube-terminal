pub mod mpv;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackQuality {
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "1080p")]
    P1080,
}

impl PlaybackQuality {
    pub fn toggle(self) -> Self {
        match self {
            Self::P720 => Self::P1080,
            Self::P1080 => Self::P720,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
        }
    }

    pub fn ytdl_format(self) -> &'static str {
        match self {
            // Avoid AV1 first; this is much faster to resolve than a long
            // avc1/mp4 preference chain and still sidesteps the codec most
            // likely to trigger video decode/render trouble on some machines.
            Self::P720 => "bestvideo[vcodec!*=av01][height<=720]+bestaudio/best[height<=720]/best",
            Self::P1080 => {
                "bestvideo[vcodec!*=av01][height<=1080]+bestaudio/best[height<=1080]/best"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlayerState {
    Stopped,
    Playing(PlayerInfo),
    Paused(PlayerInfo),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub title: String,
    pub time_pos: f64,
    pub duration: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayMode {
    Video,
    AudioOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSession {
    pub url: String,
    pub mode: PlayMode,
}
