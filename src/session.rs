use crate::app::{AppState, Tab, View};
use crate::player::{PlaybackQuality, PlaybackSession, PlayerState};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSessionState {
    pub active_tab: Tab,
    pub view: View,
    pub previous_views: Vec<View>,
    pub search_query: String,
    pub cards_selected_row: usize,
    pub cards_selected_col: usize,
    pub video_list_selected: usize,
    pub detail_selected_action: Option<usize>,
    pub channel_selected_action: Option<usize>,
    pub channel_selected_video: Option<usize>,
    pub playlist_selected_action: Option<usize>,
    pub playback_quality: PlaybackQuality,
    pub detached_player: Option<DetachedPlayerState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetachedPlayerState {
    pub session: PlaybackSession,
    pub title_hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingSessionRestore {
    pub view: View,
    pub cards_selected_row: usize,
    pub cards_selected_col: usize,
    pub video_list_selected: usize,
    pub detail_selected_action: Option<usize>,
    pub channel_selected_action: Option<usize>,
    pub channel_selected_video: Option<usize>,
    pub playlist_selected_action: Option<usize>,
}

impl PersistedSessionState {
    pub fn capture_from(state: &AppState) -> Self {
        let detached_player = state
            .current_playback
            .as_ref()
            .map(|session| DetachedPlayerState {
                session: session.clone(),
                title_hint: match &state.player_state {
                    PlayerState::Playing(info) | PlayerState::Paused(info)
                        if !info.title.is_empty() =>
                    {
                        Some(info.title.clone())
                    }
                    _ => None,
                },
            });

        Self {
            active_tab: state.tabs.active,
            view: state.view.clone(),
            previous_views: state.previous_views.clone(),
            search_query: state.search.query.clone(),
            cards_selected_row: state.cards.selected_row,
            cards_selected_col: state.cards.selected_col,
            video_list_selected: state.video_list.selected,
            detail_selected_action: state.detail.as_ref().map(|detail| detail.selected_action),
            channel_selected_action: state
                .channel_detail
                .as_ref()
                .map(|detail| detail.selected_action),
            channel_selected_video: state
                .channel_detail
                .as_ref()
                .map(|detail| detail.selected_video),
            playlist_selected_action: state
                .playlist_detail
                .as_ref()
                .map(|detail| detail.selected_action),
            playback_quality: state.playback_quality,
            detached_player,
        }
    }

    pub fn pending_restore(&self) -> PendingSessionRestore {
        PendingSessionRestore {
            view: self.view.clone(),
            cards_selected_row: self.cards_selected_row,
            cards_selected_col: self.cards_selected_col,
            video_list_selected: self.video_list_selected,
            detail_selected_action: self.detail_selected_action,
            channel_selected_action: self.channel_selected_action,
            channel_selected_video: self.channel_selected_video,
            playlist_selected_action: self.playlist_selected_action,
        }
    }
}

pub fn load(path: &Path) -> anyhow::Result<Option<PersistedSessionState>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let state = serde_json::from_str(&content)?;
    Ok(Some(state))
}

pub fn save(path: &Path, state: &PersistedSessionState) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(state)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn clear(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, View};
    use crate::player::{PlayMode, PlaybackQuality};

    #[test]
    fn roundtrip_session_state() {
        let mut state = AppState::new();
        state.tabs.active = Tab::History;
        state.view = View::Search;
        state.search.query = "rust".into();
        state.playback_quality = PlaybackQuality::P720;
        state.current_playback = Some(PlaybackSession {
            url: "https://www.youtube.com/watch?v=abc".into(),
            mode: PlayMode::Video,
        });

        let persisted = PersistedSessionState::capture_from(&state);
        let json = serde_json::to_string(&persisted).expect("serialize session state");
        let restored: PersistedSessionState =
            serde_json::from_str(&json).expect("deserialize session state");

        assert_eq!(restored.active_tab, Tab::History);
        assert_eq!(restored.view, View::Search);
        assert_eq!(restored.search_query, "rust");
        assert_eq!(restored.playback_quality, PlaybackQuality::P720);
        assert!(restored.detached_player.is_some());
    }
}
