pub mod mpv;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy)]
pub enum PlayMode {
    Video,
    AudioOnly,
}
