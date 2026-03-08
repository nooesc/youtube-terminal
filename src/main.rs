mod app;
mod auth;
mod config;
mod db;
mod event;
mod models;
mod player;
mod provider;
mod session;
mod thumbnails;
mod ui;

use app::{Action, AppState, Direction, LoadedPage, PlaybackLoadState, PopupState, Tab, View};
use auth::AuthState;
use config::Config;
use db::Database;
use event::{create_action_channel, poll_event, ActionSender};
use models::{FeedItem, ItemType, ThumbnailKey, VideoItem};
use std::path::Path;
use player::mpv::{poll_socket_state, MpvPlayer};
use player::{PlayMode, PlaybackSession};
use provider::rustypipe_provider::RustyPipeProvider;
use provider::ContentProvider;
use session::PersistedSessionState;
use thumbnails::ThumbnailCache;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // 1. Load config
    let config = Config::load()?;
    let _ = std::fs::create_dir_all(config.session_dir());
    let saved_session = session::load(&config.session_state_path()).unwrap_or(None);

    // 2. Init database
    let db = Database::open(&config.db_path())?;
    for path in db.cleanup_old_thumbnails(30, 1000).unwrap_or_default() {
        let _ = std::fs::remove_file(path);
    }

    // Ensure thumbnail cache directory exists
    let _ = std::fs::create_dir_all(config.thumbnail_dir());

    // 3. Init auth state
    let mut auth_state = AuthState::load(&config);

    // 4. Init provider
    // Note: cookies are NOT loaded into RustyPipe because its set_cookie_txt
    // validates by hitting YouTube servers, which fails with rotated sessions.
    // Cookies are only used for mpv/yt-dlp playback via the cookie file path.
    let provider = RustyPipeProvider::new(&config.rustypipe_storage_dir()).await?;
    let provider = Arc::new(provider);

    // 5. Init player
    let mut player = MpvPlayer::new(config.player_socket_path());

    // 6. Init app state
    let mut state = AppState::new();
    state.playback_quality = config.default_playback_quality;
    state.saved_searches.items = db.get_saved_searches().unwrap_or_default();
    let mut thumb_cache = ThumbnailCache::new();

    // 7. Create action channels
    let (tx, mut rx) = create_action_channel();
    spawn_player_poll_task(player.socket_path().to_path_buf(), tx.clone());

    // 8. Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 9. Restore previous state if available, otherwise load the default feed
    if let Some(saved) = saved_session {
        restore_saved_session(&saved, &mut state, &mut player, &provider, &tx, &db);
    } else {
        spawn_feed_load(&mut state, &provider, &tx, &db);
    }

    // 10. Main loop
    loop {
        // Update grid columns based on current terminal width
        state.update_columns(terminal.size()?.width);

        // Render
        terminal.draw(|f| ui::render(f, &state, &thumb_cache))?;

        // Poll crossterm events
        if let Some(action) = poll_event(&state) {
            // Clear command status message on any key press
            if !state.command.active && state.command.message.is_some() {
                state.command.message = None;
            }
            handle_action(
                action,
                &mut state,
                &mut player,
                &db,
                &config,
                &mut auth_state,
                &provider,
                &tx,
                &mut thumb_cache,
            );
        }

        // Drain async actions from channel
        while let Ok(action) = rx.try_recv() {
            handle_action(
                action,
                &mut state,
                &mut player,
                &db,
                &config,
                &mut auth_state,
                &provider,
                &tx,
                &mut thumb_cache,
            );
        }

        if state.should_quit {
            break;
        }
    }

    // 11. Cleanup
    let _ = persist_session(&state, &config);
    if state.stop_player_on_exit {
        player.stop();
    } else {
        player.detach();
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_action(
    action: Action,
    state: &mut AppState,
    player: &mut MpvPlayer,
    db: &Database,
    config: &Config,
    auth_state: &mut AuthState,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
    thumb_cache: &mut ThumbnailCache,
) {
    match action {
        Action::SubmitSearch(ref query) => {
            let query = query.clone();
            state.dispatch(action);
            spawn_search_load(state, &query, provider, tx);
        }
        Action::FilterCycleUp | Action::FilterCycleDown | Action::ResetFilters => {
            state.dispatch(action);
            // Auto-resubmit search with new filters
            if !state.search.query.is_empty() {
                let query = state.search.query.clone();
                state.loading.search_request_id += 1;
                state.loading.search_loading = true;
                state.video_list.items.clear();
                state.video_list.selected = 0;
                spawn_search_load(state, &query, provider, tx);
            }
        }
        Action::SubmitCommand(ref cmd) => {
            let cmd = cmd.trim().to_string();
            state.dispatch(action);
            execute_command(&cmd, state, player, config, auth_state);
        }
        Action::Select => {
            // Determine what to load based on current view
            match &state.view {
                View::Search => {
                    if let Some(item) = state.selected_list_item().cloned() {
                        handle_item_select(&item, state, provider, tx, db);
                    }
                }
                View::Home => {
                    if state.tabs.active == Tab::SavedSearches {
                        if let Some(saved) = state.saved_searches.items.get(state.saved_searches.selected).cloned() {
                            let _ = db.update_last_run(saved.id);
                            state.search.query = saved.query.clone();
                            state.search.cursor = saved.query.len();
                            state.search.filter.sort = saved.sort;
                            state.search.filter.date = saved.date;
                            state.search.filter.item_type = saved.item_type;
                            state.search.filter.length = saved.length;
                            state.previous_views.push(state.view.clone());
                            state.view = View::Search;
                            state.loading.search_request_id += 1;
                            state.loading.search_loading = true;
                            state.video_list.items.clear();
                            state.video_list.selected = 0;
                            spawn_search_load(state, &saved.query, provider, tx);
                            state.saved_searches.items = db.get_saved_searches().unwrap_or_default();
                        }
                    } else if state.tabs.active == Tab::Subscriptions {
                        // Select from subscription channel list
                        let channel_id = state
                            .subscription_channels
                            .get(state.cards.selected_row)
                            .map(|c| c.id.clone());
                        if let Some(id) = channel_id {
                            spawn_channel_load(state, &id, provider, tx);
                        }
                    } else if let Some(item) = state.selected_card_item().cloned() {
                        handle_item_select(&item, state, provider, tx, db);
                    }
                }
                View::VideoDetail(_) => {
                    if let Some(detail_state) = state.detail.as_ref() {
                        let detail = detail_state.detail.clone();
                        let video_id = detail.item.id.clone();
                        match detail_state.selected_action {
                            0 => {
                                let url = format!("https://www.youtube.com/watch?v={}", video_id);
                                if let Err(e) = start_playback(
                                    state,
                                    player,
                                    config,
                                    auth_state,
                                    tx,
                                    &url,
                                    PlayMode::Video,
                                    detail.item.title.clone(),
                                ) {
                                    state.command.message =
                                        Some(format!("Playback error: {} (is mpv installed?)", e));
                                } else {
                                    record_history(db, &detail);
                                }
                            }
                            1 => {
                                let url = format!("https://www.youtube.com/watch?v={}", video_id);
                                if let Err(e) = start_playback(
                                    state,
                                    player,
                                    config,
                                    auth_state,
                                    tx,
                                    &url,
                                    PlayMode::AudioOnly,
                                    detail.item.title.clone(),
                                ) {
                                    state.command.message =
                                        Some(format!("Playback error: {} (is mpv installed?)", e));
                                } else {
                                    record_history(db, &detail);
                                }
                            }
                            2 => {
                                // Open Channel -- navigate to channel detail
                                let channel_id = detail_state.detail.item.channel_id.clone();
                                if !channel_id.is_empty() {
                                    spawn_channel_load(state, &channel_id, provider, tx);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                View::PlaylistDetail(_) => {
                    if let Some(detail_state) = state.playlist_detail.as_ref() {
                        let playlist_id = detail_state.detail.item.id.clone();
                        match detail_state.selected_action {
                            0 => {
                                let url = format!(
                                    "https://www.youtube.com/playlist?list={}",
                                    playlist_id
                                );
                                if let Err(e) = start_playback(
                                    state,
                                    player,
                                    config,
                                    auth_state,
                                    tx,
                                    &url,
                                    PlayMode::Video,
                                    detail_state.detail.item.title.clone(),
                                ) {
                                    state.command.message =
                                        Some(format!("Playback error: {} (is mpv installed?)", e));
                                }
                            }
                            1 => {
                                let url = format!(
                                    "https://www.youtube.com/playlist?list={}",
                                    playlist_id
                                );
                                if let Err(e) = start_playback(
                                    state,
                                    player,
                                    config,
                                    auth_state,
                                    tx,
                                    &url,
                                    PlayMode::AudioOnly,
                                    detail_state.detail.item.title.clone(),
                                ) {
                                    state.command.message =
                                        Some(format!("Playback error: {} (is mpv installed?)", e));
                                }
                            }
                            2 => {
                                let channel_id = detail_state.detail.item.channel_id.clone();
                                if !channel_id.is_empty() {
                                    spawn_channel_load(state, &channel_id, provider, tx);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                View::ChannelDetail(_) => {
                    if let Some(detail_state) = &state.channel_detail {
                        match detail_state.selected_action {
                            0 => {
                                // Subscribe/Unsubscribe toggle
                                let channel = detail_state.detail.item.clone();
                                if detail_state.is_subscribed {
                                    if db.unsubscribe(&channel.id).is_ok() {
                                        state.command.message =
                                            Some(format!("Unsubscribed from {}", channel.name));
                                        if let Some(ref mut cd) = state.channel_detail {
                                            cd.is_subscribed = false;
                                        }
                                    }
                                } else if db.subscribe(&channel).is_ok() {
                                    state.command.message =
                                        Some(format!("Subscribed to {}", channel.name));
                                    if let Some(ref mut cd) = state.channel_detail {
                                        cd.is_subscribed = true;
                                    }
                                }
                            }
                            1 => {
                                // Select a video from the channel's video list
                                let video_idx = detail_state.selected_video;
                                if let Some(video) = detail_state.detail.videos.get(video_idx) {
                                    let v = video.clone();
                                    spawn_detail_load(state, &v, provider, tx, db);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            // Don't dispatch Select to state -- we handle it entirely here
        }
        Action::SwitchTab(ref tab) => {
            let tab = *tab;
            state.dispatch(action);
            if tab == Tab::SavedSearches {
                state.saved_searches.items = db.get_saved_searches().unwrap_or_default();
            } else {
                spawn_feed_load(state, provider, tx, db);
            }
        }
        Action::Quit => {
            refresh_player_geometry(state, player);
            state.stop_player_on_exit = false;
            state.dispatch(action);
        }
        Action::TogglePause => {
            let _ = player.toggle_pause();
            // Update player state immediately
            if let Ok(ps) = player.poll_state() {
                state.dispatch(Action::PlayerStateUpdate(ps));
            }
        }
        Action::Seek(secs) => {
            let _ = player.seek(secs);
        }
        Action::VolumeUp => {
            if let Ok(val) = player.get_property("volume") {
                let vol = val.as_f64().unwrap_or(100.0);
                let _ = player.set_volume((vol + 5.0).min(150.0));
            }
        }
        Action::VolumeDown => {
            if let Ok(val) = player.get_property("volume") {
                let vol = val.as_f64().unwrap_or(100.0);
                let _ = player.set_volume((vol - 5.0).max(0.0));
            }
        }
        Action::TogglePlaybackQuality => {
            state.playback_quality = state.playback_quality.toggle();
            let reloaded =
                reload_current_playback(state, player, config, auth_state, tx).unwrap_or(false);
            let suffix = if reloaded {
                " (reloaded current media)"
            } else {
                ""
            };
            state.command.message = Some(format!(
                "Playback quality: {}{}",
                state.playback_quality.label(),
                suffix
            ));
        }
        Action::StopPlayer => {
            stop_playback(player, state);
            let _ = session::clear(&config.session_state_path());
            state.command.message = Some("Stopped player".into());
        }
        Action::StopPlayerAndQuit => {
            stop_playback(player, state);
            let _ = session::clear(&config.session_state_path());
            state.stop_player_on_exit = false;
            state.should_quit = true;
        }
        Action::PlayVideo(ref id) => {
            let url = format!("https://www.youtube.com/watch?v={}", id);
            if let Err(e) = start_playback(
                state,
                player,
                config,
                auth_state,
                tx,
                &url,
                PlayMode::Video,
                "video".to_string(),
            ) {
                state.command.message = Some(format!("Playback error: {} (is mpv installed?)", e));
            }
        }
        Action::PlayAudio(ref id) => {
            let url = format!("https://www.youtube.com/watch?v={}", id);
            if let Err(e) = start_playback(
                state,
                player,
                config,
                auth_state,
                tx,
                &url,
                PlayMode::AudioOnly,
                "audio stream".to_string(),
            ) {
                state.command.message = Some(format!("Playback error: {} (is mpv installed?)", e));
            }
        }
        Action::Navigate(dir) => {
            state.dispatch(action);
            spawn_thumbnail_downloads(state, tx, config, thumb_cache, db);
            if matches!(dir, Direction::Down) {
                check_load_more(state, provider, tx);
            }
        }
        Action::FeedLoaded(_, _) | Action::SearchResults(_, _) => {
            state.dispatch(action);
            spawn_thumbnail_downloads(state, tx, config, thumb_cache, db);
        }
        Action::AppendFeed(_, _) | Action::AppendSearch(_, _) => {
            state.dispatch(action);
            spawn_thumbnail_downloads(state, tx, config, thumb_cache, db);
        }
        Action::DetailLoaded(_, ref detail) => {
            if let Ok(json) = serde_json::to_string(detail) {
                let _ = db.set_cached_metadata(&detail.item.id, &json);
            }
            // Load detail-sized thumbnail for the video detail page
            let thumb_key = models::ThumbnailKey {
                item_type: models::ItemType::Video,
                item_id: detail.item.id.clone(),
            };
            if thumb_cache.get_detail(&thumb_key).is_none() {
                if let Ok(Some(path)) = db.get_thumbnail_path(&thumb_key) {
                    if path.exists() {
                        let _ = thumb_cache.load_detail(
                            &thumb_key,
                            &path,
                            ui::video_detail::DETAIL_THUMB_W,
                            ui::video_detail::DETAIL_THUMB_H,
                        );
                    }
                }
            }
            state.dispatch(action);
            apply_pending_restore(state);
        }
        Action::ChannelDetailLoaded(_, ref detail) => {
            let is_subbed = db.is_subscribed(&detail.item.id).unwrap_or(false);
            // Refresh subscriber count in DB if subscribed (search results may have wrong counts)
            if is_subbed {
                if let Some(count) = detail.item.subscriber_count {
                    let _ = db.update_subscriber_count(&detail.item.id, count);
                    // Also update the in-memory subscription list
                    for ch in &mut state.subscription_channels {
                        if ch.id == detail.item.id {
                            ch.subscriber_count = Some(count);
                        }
                    }
                }
            }
            state.dispatch(action);
            if let Some(ref mut cd) = state.channel_detail {
                cd.is_subscribed = is_subbed;
            }
            apply_pending_restore(state);
        }
        Action::PlaylistDetailLoaded(_, _) => {
            state.dispatch(action);
            apply_pending_restore(state);
        }
        Action::ThumbnailReady(ref key, ref path) => {
            // Load the downloaded image into the render cache
            let tw = (ui::card_grid::CARD_WIDTH - 2) as u32;
            let th = ui::card_grid::THUMB_HEIGHT as u32;
            if thumb_cache.load(key, path, tw, th).is_ok() {
                let _ = db.set_thumbnail_path(key, path);
                // Also load as avatar for subscription list
                if key.item_type == ItemType::Channel {
                    let avatar_size = ui::subscription_list::AVATAR_SIZE as u32;
                    let _ = thumb_cache.load_avatar(key, path, avatar_size);
                }
                state.dispatch(action);
            } else {
                let _ = std::fs::remove_file(path);
                state.dispatch(Action::ThumbnailFailed(key.clone()));
            }
        }
        Action::ThumbnailFailed(_) => {
            state.dispatch(action);
        }
        Action::Subscribe(ref channel) => match db.subscribe(channel) {
            Ok(()) => {
                state.command.message = Some(format!("Subscribed to {}", channel.name));
                if let Some(ref mut cd) = state.channel_detail {
                    if cd.detail.item.id == channel.id {
                        cd.is_subscribed = true;
                    }
                }
            }
            Err(e) => {
                state.command.message = Some(format!("Subscribe error: {}", e));
            }
        },
        Action::Unsubscribe(ref channel_id) => {
            let name = state
                .channel_detail
                .as_ref()
                .filter(|cd| cd.detail.item.id == *channel_id)
                .map(|cd| cd.detail.item.name.clone())
                .unwrap_or_else(|| channel_id.clone());
            match db.unsubscribe(channel_id) {
                Ok(()) => {
                    state.command.message = Some(format!("Unsubscribed from {}", name));
                    if let Some(ref mut cd) = state.channel_detail {
                        if cd.detail.item.id == *channel_id {
                            cd.is_subscribed = false;
                        }
                    }
                }
                Err(e) => {
                    state.command.message = Some(format!("Unsubscribe error: {}", e));
                }
            }
        }
        Action::RefreshSubscriberCount(ref channel_id, count) => {
            let _ = db.update_subscriber_count(channel_id, count);
            state.dispatch(action);
        }
        Action::SubscribeSelected => {
            let channel = match &state.view {
                View::Search => {
                    if let Some(FeedItem::Channel(c)) = state.selected_list_item() {
                        Some(c.clone())
                    } else {
                        None
                    }
                }
                View::Home => {
                    if let Some(FeedItem::Channel(c)) = state.selected_card_item() {
                        Some(c.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(channel) = channel {
                let is_subbed = db.is_subscribed(&channel.id).unwrap_or(false);
                if is_subbed {
                    if db.unsubscribe(&channel.id).is_ok() {
                        state.command.message = Some(format!("Unsubscribed from {}", channel.name));
                    }
                } else if db.subscribe(&channel).is_ok() {
                    state.command.message = Some(format!("Subscribed to {}", channel.name));
                }
            } else {
                state.command.message = Some("Select a channel to subscribe".into());
            }
        }
        Action::PopupSubmit => {
            let popup = state.popup.take();
            match popup {
                Some(PopupState::SaveSearch { ref input, .. }) => {
                    if !input.is_empty() {
                        let name = input.clone();
                        let filter = &state.search.filter;
                        match db.save_search(
                            &name,
                            &state.search.query,
                            filter.sort,
                            filter.date,
                            filter.item_type,
                            filter.length,
                        ) {
                            Ok(_) => {
                                state.command.message =
                                    Some(format!("Saved search \"{}\"", name));
                                state.saved_searches.items =
                                    db.get_saved_searches().unwrap_or_default();
                            }
                            Err(e) => {
                                state.command.message =
                                    Some(format!("Save error: {}", e));
                            }
                        }
                    }
                }
                Some(PopupState::ConfirmDelete { id, ref name }) => {
                    let name = name.clone();
                    match db.delete_saved_search(id) {
                        Ok(()) => {
                            state.command.message = Some(format!("Deleted \"{}\"", name));
                            state.saved_searches.items =
                                db.get_saved_searches().unwrap_or_default();
                            let max = state.saved_searches.items.len().saturating_sub(1);
                            state.saved_searches.selected =
                                state.saved_searches.selected.min(max);
                        }
                        Err(e) => {
                            state.command.message = Some(format!("Delete error: {}", e));
                        }
                    }
                }
                Some(PopupState::Rename { id, ref input, .. }) => {
                    if !input.is_empty() {
                        let new_name = input.clone();
                        match db.rename_saved_search(id, &new_name) {
                            Ok(()) => {
                                state.command.message =
                                    Some(format!("Renamed to \"{}\"", new_name));
                                state.saved_searches.items =
                                    db.get_saved_searches().unwrap_or_default();
                            }
                            Err(e) => {
                                state.command.message =
                                    Some(format!("Rename error: {}", e));
                            }
                        }
                    }
                }
                None => {}
            }
        }
        _ => {
            // All other actions go through normal dispatch
            let needs_restore = matches!(
                &action,
                Action::FeedLoaded(_, _)
                    | Action::SearchResults(_, _)
                    | Action::PlayerStateUpdate(_)
                    | Action::PlaybackLoadSlow(_)
            );
            state.dispatch(action);
            if needs_restore {
                apply_pending_restore(state);
            }
        }
    }
}

fn restore_saved_session(
    saved: &PersistedSessionState,
    state: &mut AppState,
    player: &mut MpvPlayer,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
    db: &Database,
) {
    state.tabs.active = saved.active_tab;
    state.view = saved.view.clone();
    state.previous_views = saved.previous_views.clone();
    state.search.query = saved.search_query.clone();
    state.search.cursor = state.search.query.len();
    state.search.focused = false;
    state.playback_quality = saved.playback_quality;
    state.last_mpv_geometry = saved.window_geometry.clone();
    state.pending_restore = Some(saved.pending_restore());

    if let Some(detached) = saved.detached_player.clone() {
        if player.attach_if_running() {
            state.current_playback = Some(detached.session);
            state.dispatch(Action::PlayerStateUpdate(poll_socket_state(
                player.socket_path(),
            )));
        }
    }

    match &saved.view {
        View::Home => {
            spawn_feed_load(state, provider, tx, db);
            // SavedSearches tab has no async load, apply restore directly
            if saved.active_tab == Tab::SavedSearches {
                let max = state.saved_searches.items.len().saturating_sub(1);
                state.saved_searches.selected = saved.saved_search_selected.min(max);
                state.pending_restore = None;
            }
        }
        View::Search => {
            if saved.search_query.is_empty() {
                state.view = View::Home;
                spawn_feed_load(state, provider, tx, db);
            } else {
                spawn_search_load(state, &saved.search_query, provider, tx);
            }
        }
        View::VideoDetail(video_id) => {
            spawn_detail_load_by_id(state, video_id, provider, tx, db);
        }
        View::ChannelDetail(channel_id) => {
            spawn_channel_load(state, channel_id, provider, tx);
        }
        View::PlaylistDetail(playlist_id) => {
            spawn_playlist_load(state, playlist_id, provider, tx);
        }
    }
}

fn persist_session(state: &AppState, config: &Config) -> anyhow::Result<()> {
    let mut persisted = PersistedSessionState::capture_from(state);
    if state.stop_player_on_exit {
        persisted.detached_player = None;
    }
    session::save(&config.session_state_path(), &persisted)
}

fn apply_pending_restore(state: &mut AppState) {
    let Some(restore) = state.pending_restore.clone() else {
        return;
    };

    let restored = match &restore.view {
        View::Home if matches!(state.view, View::Home) => {
            if state.tabs.active == Tab::SavedSearches {
                let max = state.saved_searches.items.len().saturating_sub(1);
                state.saved_searches.selected = restore.saved_search_selected.min(max);
            } else if state.tabs.active == Tab::Subscriptions {
                let max = state.subscription_channels.len().saturating_sub(1);
                state.cards.selected_row = restore.cards_selected_row.min(max);
                state.cards.selected_col = 0;
            } else {
                let total = state.cards.items.len();
                if total == 0 {
                    state.cards.selected_row = 0;
                    state.cards.selected_col = 0;
                } else {
                    let cols = state.cards.columns.max(1);
                    let max_row = (total - 1) / cols;
                    state.cards.selected_row = restore.cards_selected_row.min(max_row);
                    let index = state.cards.selected_row * cols;
                    let row_len = (total - index).min(cols);
                    state.cards.selected_col =
                        restore.cards_selected_col.min(row_len.saturating_sub(1));
                }
            }
            true
        }
        View::Search if matches!(state.view, View::Search) => {
            let max = state.video_list.items.len().saturating_sub(1);
            state.video_list.selected = restore.video_list_selected.min(max);
            true
        }
        View::VideoDetail(id) => match &state.view {
            View::VideoDetail(current_id) if current_id == id => {
                if let Some(detail) = state.detail.as_mut() {
                    detail.selected_action = restore.detail_selected_action.unwrap_or(0).min(2);
                }
                true
            }
            _ => false,
        },
        View::ChannelDetail(id) => match &state.view {
            View::ChannelDetail(current_id) if current_id == id => {
                if let Some(detail) = state.channel_detail.as_mut() {
                    detail.selected_action = restore.channel_selected_action.unwrap_or(0).min(1);
                    let max_video = detail.detail.videos.len().saturating_sub(1);
                    detail.selected_video =
                        restore.channel_selected_video.unwrap_or(0).min(max_video);
                }
                true
            }
            _ => false,
        },
        View::PlaylistDetail(id) => match &state.view {
            View::PlaylistDetail(current_id) if current_id == id => {
                if let Some(detail) = state.playlist_detail.as_mut() {
                    detail.selected_action = restore.playlist_selected_action.unwrap_or(0).min(2);
                }
                true
            }
            _ => false,
        },
        _ => false,
    };

    if restored {
        state.pending_restore = None;
    }
}

fn stop_playback(player: &mut MpvPlayer, state: &mut AppState) {
    refresh_player_geometry(state, player);
    player.stop();
    state.playback_loading = None;
    state.current_playback = None;
    state.player_state = player::PlayerState::Stopped;
}

fn spawn_search_load(
    state: &mut AppState,
    query: &str,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
) {
    let tx = tx.clone();
    let provider = Arc::clone(provider);
    let req_id = state.loading.search_request_id;
    let query = query.to_string();
    let options = build_search_options(&state.search.filter);
    tokio::spawn(async move {
        let result = if options.is_some() {
            provider.search_filtered(&query, &options.unwrap()).await
        } else {
            provider.search(&query, None).await
        };
        match result {
            Ok(page) => {
                let _ = tx.send(Action::SearchResults(req_id, page));
            }
            Err(e) => {
                let _ = tx.send(Action::ShowError(format!("Search error: {}", e)));
            }
        }
    });
}

fn build_search_options(
    filter: &app::SearchFilterState,
) -> Option<provider::SearchOptions> {
    use app::{SearchDate, SearchItemType, SearchLength, SearchSort};
    if !filter.has_filters() {
        return None;
    }
    Some(provider::SearchOptions {
        sort: if filter.sort != SearchSort::Relevance {
            Some(filter.sort)
        } else {
            None
        },
        date: if filter.date != SearchDate::Any {
            Some(filter.date)
        } else {
            None
        },
        item_type: if filter.item_type != SearchItemType::All {
            Some(filter.item_type)
        } else {
            None
        },
        length: if filter.length != SearchLength::Any {
            Some(filter.length)
        } else {
            None
        },
    })
}

fn start_playback(
    state: &mut AppState,
    player: &mut MpvPlayer,
    config: &Config,
    auth_state: &AuthState,
    tx: &ActionSender,
    url: &str,
    mode: PlayMode,
    label: String,
) -> anyhow::Result<()> {
    refresh_player_geometry(state, player);
    player.play(
        url,
        mode,
        state.playback_quality,
        config,
        state.last_mpv_geometry.as_deref(),
        auth_state.cookie_path(),
    )?;
    state.loading.playback_request_id += 1;
    let request_id = state.loading.playback_request_id;
    state.playback_loading = Some(PlaybackLoadState {
        request_id,
        label,
        started_at: std::time::Instant::now(),
        slow: false,
    });
    state.current_playback = Some(PlaybackSession {
        url: url.to_string(),
        mode,
    });
    let tx = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        let _ = tx.send(Action::PlaybackLoadSlow(request_id));
    });
    Ok(())
}

fn refresh_player_geometry(state: &mut AppState, player: &mut MpvPlayer) {
    if let Ok(geometry) = player.window_geometry() {
        if !geometry.trim().is_empty() {
            state.last_mpv_geometry = Some(geometry);
        }
    }
}

fn reload_current_playback(
    state: &mut AppState,
    player: &mut MpvPlayer,
    config: &Config,
    auth_state: &AuthState,
    tx: &ActionSender,
) -> anyhow::Result<bool> {
    let Some(session) = state.current_playback.clone() else {
        return Ok(false);
    };

    let (resume_at, restore_paused) = match &state.player_state {
        player::PlayerState::Playing(info) => (Some(info.time_pos), false),
        player::PlayerState::Paused(info) => (Some(info.time_pos), true),
        player::PlayerState::Stopped => return Ok(false),
    };
    let label = match &state.player_state {
        player::PlayerState::Playing(info) | player::PlayerState::Paused(info)
            if !info.title.is_empty() =>
        {
            info.title.clone()
        }
        _ => "current media".to_string(),
    };

    start_playback(
        state,
        player,
        config,
        auth_state,
        tx,
        &session.url,
        session.mode,
        label,
    )?;

    if let Some(seconds) = resume_at.filter(|seconds| *seconds > 0.0) {
        let _ = player.seek_to(seconds);
    }
    if restore_paused {
        let _ = player.toggle_pause();
    }

    Ok(true)
}

fn spawn_feed_load(
    state: &mut AppState,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
    db: &Database,
) {
    // SavedSearches tab uses local DB, no feed load needed
    if state.tabs.active == Tab::SavedSearches {
        return;
    }

    state.loading.feed_request_id += 1;
    state.loading.feed_loading = true;
    let req_id = state.loading.feed_request_id;

    // History tab: load synchronously from local SQLite DB
    if state.tabs.active == Tab::History {
        let history = db.get_history(50, 0).unwrap_or_default();
        let page = LoadedPage::History(models::FeedPage {
            items: history,
            continuation: None,
        });
        let _ = tx.send(Action::FeedLoaded(req_id, Box::new(page)));
        return;
    }

    // Subscriptions tab: load from local SQLite DB
    if state.tabs.active == Tab::Subscriptions {
        let channels = db.get_subscriptions().unwrap_or_default();
        // Spawn background subscriber count refresh for channels with suspicious counts
        spawn_subscriber_count_refresh(&channels, provider, tx);
        let page = LoadedPage::Subscriptions(models::FeedPage {
            items: channels,
            continuation: None,
        });
        let _ = tx.send(Action::FeedLoaded(req_id, Box::new(page)));
        return;
    }

    // For You tab: aggregate videos from local subscriptions
    if state.tabs.active == Tab::ForYou {
        let channel_ids = db.get_subscribed_channel_ids().unwrap_or_default();
        if channel_ids.is_empty() {
            let page = LoadedPage::Home(models::FeedPage {
                items: Vec::new(),
                continuation: None,
            });
            let _ = tx.send(Action::FeedLoaded(req_id, Box::new(page)));
            return;
        }

        let tx = tx.clone();
        let provider = Arc::clone(provider);
        tokio::spawn(async move {
            let mut handles = Vec::new();
            for channel_id in channel_ids {
                let provider = Arc::clone(&provider);
                handles.push(tokio::spawn(async move {
                    provider
                        .channel_latest_videos(&channel_id)
                        .await
                        .unwrap_or_default()
                }));
            }

            let mut all_videos: Vec<models::VideoItem> = Vec::new();
            for handle in handles {
                if let Ok(videos) = handle.await {
                    all_videos.extend(videos);
                }
            }

            // Sort by publish date descending (newest first)
            all_videos.sort_by(|a, b| b.published.cmp(&a.published));

            let page = LoadedPage::Home(models::FeedPage {
                items: all_videos
                    .into_iter()
                    .map(models::FeedItem::Video)
                    .collect(),
                continuation: None,
            });
            let _ = tx.send(Action::FeedLoaded(req_id, Box::new(page)));
        });
    }
}

fn spawn_detail_load(
    state: &mut AppState,
    video: &VideoItem,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
    db: &Database,
) {
    state.loading.detail_request_id += 1;
    state.loading.detail_loading = true;
    let req_id = state.loading.detail_request_id;

    if let Ok(Some(json)) = db.get_cached_metadata(&video.id) {
        if let Ok(detail) = serde_json::from_str::<models::VideoDetail>(&json) {
            let _ = tx.send(Action::DetailLoaded(req_id, detail));
            return;
        }
    }
    let tx = tx.clone();
    let provider = Arc::clone(provider);
    let fallback = video.clone();

    tokio::spawn(async move {
        match provider.video_detail(&fallback.id).await {
            Ok(mut detail) => {
                if detail.item.duration.is_none() {
                    detail.item.duration = fallback.duration;
                }
                if detail.item.thumbnail_url.is_empty() {
                    detail.item.thumbnail_url = fallback.thumbnail_url.clone();
                }
                let _ = tx.send(Action::DetailLoaded(req_id, detail));
            }
            Err(e) => {
                let _ = tx.send(Action::ShowError(format!("Detail error: {}", e)));
            }
        }
    });
}

fn spawn_detail_load_by_id(
    state: &mut AppState,
    video_id: &str,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
    db: &Database,
) {
    state.loading.detail_request_id += 1;
    state.loading.detail_loading = true;
    let req_id = state.loading.detail_request_id;

    if let Ok(Some(json)) = db.get_cached_metadata(video_id) {
        if let Ok(detail) = serde_json::from_str::<models::VideoDetail>(&json) {
            let _ = tx.send(Action::DetailLoaded(req_id, detail));
            return;
        }
    }

    let tx = tx.clone();
    let provider = Arc::clone(provider);
    let id = video_id.to_string();
    tokio::spawn(async move {
        match provider.video_detail(&id).await {
            Ok(detail) => {
                let _ = tx.send(Action::DetailLoaded(req_id, detail));
            }
            Err(e) => {
                let _ = tx.send(Action::ShowError(format!("Detail error: {}", e)));
            }
        }
    });
}

/// Refresh subscriber counts in the background for subscriptions.
/// Fetches channel details and sends RefreshSubscriberCount actions.
fn spawn_subscriber_count_refresh(
    channels: &[models::ChannelItem],
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
) {
    for channel in channels {
        let tx = tx.clone();
        let provider = Arc::clone(provider);
        let channel_id = channel.id.clone();
        tokio::spawn(async move {
            if let Ok(detail) = provider.channel(&channel_id).await {
                if let Some(count) = detail.item.subscriber_count {
                    let _ = tx.send(Action::RefreshSubscriberCount(channel_id, count));
                }
            }
        });
    }
}

fn spawn_channel_load(
    state: &mut AppState,
    channel_id: &str,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
) {
    state.loading.detail_request_id += 1;
    state.loading.detail_loading = true;
    let req_id = state.loading.detail_request_id;
    let tx = tx.clone();
    let provider = Arc::clone(provider);
    let id = channel_id.to_string();

    tokio::spawn(async move {
        match provider.channel(&id).await {
            Ok(detail) => {
                let _ = tx.send(Action::ChannelDetailLoaded(req_id, detail));
            }
            Err(e) => {
                let _ = tx.send(Action::ShowError(format!("Channel error: {}", e)));
            }
        }
    });
}

fn spawn_playlist_load(
    state: &mut AppState,
    playlist_id: &str,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
) {
    state.loading.detail_request_id += 1;
    state.loading.detail_loading = true;
    let req_id = state.loading.detail_request_id;
    let tx = tx.clone();
    let provider = Arc::clone(provider);
    let id = playlist_id.to_string();

    tokio::spawn(async move {
        match provider.playlist(&id).await {
            Ok(detail) => {
                let _ = tx.send(Action::PlaylistDetailLoaded(req_id, detail));
            }
            Err(e) => {
                let _ = tx.send(Action::ShowError(format!("Playlist error: {}", e)));
            }
        }
    });
}

fn check_load_more(state: &mut AppState, provider: &Arc<RustyPipeProvider>, tx: &ActionSender) {
    match &state.view {
        View::Home => {
            // For You, Subscriptions, and History are loaded without pagination
        }
        View::Search => {
            if state.loading.search_loading || state.loading.loading_more_search {
                return;
            }
            let total = state.video_list.items.len();
            let idx = state.video_list.selected;
            // Trigger when within last 3 items
            if total > 0 && idx >= total.saturating_sub(3) {
                if let Some(ctoken) = state.video_list.continuation.clone() {
                    state.loading.loading_more_search = true;
                    let req_id = state.loading.search_request_id;
                    let query = state.search.query.clone();
                    let tx = tx.clone();
                    let provider = Arc::clone(provider);

                    tokio::spawn(async move {
                        match provider.search(&query, Some(&ctoken)).await {
                            Ok(page) => {
                                let _ = tx.send(Action::AppendSearch(req_id, page));
                            }
                            Err(e) => {
                                let _ = tx.send(Action::ShowError(format!(
                                    "Search continuation error: {}",
                                    e
                                )));
                            }
                        }
                    });
                }
            }
        }
        _ => {}
    }
}

fn handle_item_select(
    item: &FeedItem,
    state: &mut AppState,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
    db: &Database,
) {
    match item {
        FeedItem::Video(v) | FeedItem::Short(v) => {
            spawn_detail_load(state, v, provider, tx, db);
        }
        FeedItem::Channel(c) => {
            spawn_channel_load(state, &c.id, provider, tx);
        }
        FeedItem::Playlist(p) => {
            spawn_playlist_load(state, &p.id, provider, tx);
        }
    }
}

fn spawn_thumbnail_downloads(
    state: &mut AppState,
    tx: &ActionSender,
    config: &Config,
    thumb_cache: &ThumbnailCache,
    db: &Database,
) {
    // Handle subscription channel avatars
    if state.view == View::Home && state.tabs.active == Tab::Subscriptions {
        let cache_dir = config.thumbnail_dir();
        if let Some(channel) = state.subscription_channels.get(state.cards.selected_row) {
            let key = ThumbnailKey {
                item_type: ItemType::Channel,
                item_id: channel.id.clone(),
            };
            if !state.loading.thumbnail_loading.contains(&key)
                && thumb_cache.get_avatar(&key).is_none()
                && !channel.thumbnail_url.is_empty()
            {
                download_thumbnail_if_needed(
                    state,
                    tx,
                    &cache_dir,
                    db,
                    key,
                    channel.thumbnail_url.clone(),
                );
            }
        }
        return;
    }

    let item_indexes: Vec<usize> = match &state.view {
        View::Home => {
            let cols = state.cards.columns.max(1);
            let start_row = state.cards.selected_row.saturating_sub(1);
            let end_row = state.cards.selected_row + 3;
            let start = start_row * cols;
            let end = (end_row * cols).min(state.cards.items.len());
            (start..end).collect()
        }
        View::Search => {
            let start = state.video_list.selected.saturating_sub(5);
            let end = (state.video_list.selected + 15).min(state.video_list.items.len());
            (start..end).collect()
        }
        _ => return,
    };

    let cache_dir = config.thumbnail_dir();
    for item_idx in item_indexes {
        let item = match &state.view {
            View::Home => &state.cards.items[item_idx],
            View::Search => &state.video_list.items[item_idx],
            _ => return,
        };
        let key = item.thumbnail_key();
        if state.loading.thumbnail_loading.contains(&key) || thumb_cache.get(&key).is_some() {
            continue;
        }
        let url = item.thumbnail_url().to_string();
        if url.is_empty() {
            continue;
        }

        download_thumbnail_if_needed(state, tx, &cache_dir, db, key, url);
    }
}

fn download_thumbnail_if_needed(
    state: &mut AppState,
    tx: &ActionSender,
    cache_dir: &Path,
    db: &Database,
    key: ThumbnailKey,
    url: String,
) {
    if let Ok(Some(existing_path)) = db.get_thumbnail_path(&key) {
        if existing_path.exists() {
            let _ = tx.send(Action::ThumbnailReady(key, existing_path));
            return;
        }
    }

    // Check if file already exists on disk
    let filename = format!(
        "{}_{}.jpg",
        match key.item_type {
            ItemType::Video => "video",
            ItemType::Channel => "channel",
            ItemType::Playlist => "playlist",
        },
        key.item_id
    );
    let cached_path = cache_dir.join(&filename);
    if cached_path.exists() {
        let _ = tx.send(Action::ThumbnailReady(key, cached_path));
        return;
    }

    state.loading.thumbnail_loading.insert(key.clone());
    let tx = tx.clone();
    let key_clone = key.clone();
    let cache_dir = cache_dir.to_path_buf();
    tokio::spawn(async move {
        match thumbnails::download_thumbnail(&url, &key_clone, &cache_dir).await {
            Ok(path) => {
                let _ = tx.send(Action::ThumbnailReady(key_clone, path));
            }
            Err(_) => {
                let _ = tx.send(Action::ThumbnailFailed(key_clone));
            }
        }
    });
}

fn record_history(db: &Database, detail: &models::VideoDetail) {
    let _ = db.add_to_history(
        &detail.item.id,
        &detail.item.title,
        &detail.item.channel,
        &detail.item.channel_id,
        &detail.item.thumbnail_url,
        detail.item.duration,
    );
}

fn execute_command(
    cmd: &str,
    state: &mut AppState,
    player: &mut MpvPlayer,
    config: &Config,
    auth_state: &mut AuthState,
) {
    if cmd == "q" || cmd == "quit" {
        refresh_player_geometry(state, player);
        state.stop_player_on_exit = false;
        state.should_quit = true;
        return;
    }

    if cmd == "stop-player" {
        stop_playback(player, state);
        let _ = session::clear(&config.session_state_path());
        state.command.message = Some("Stopped player".into());
        return;
    }

    if let Some(path_str) = cmd.strip_prefix("import-cookies ") {
        let path_str = path_str.trim();
        // Expand ~ to home directory
        let expanded = if let Some(rest) = path_str.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(rest)
            } else {
                std::path::PathBuf::from(path_str)
            }
        } else {
            std::path::PathBuf::from(path_str)
        };
        let source = expanded.as_path();
        let dest = config.cookie_path();

        match auth::cookies::import_cookie_file(source, &dest) {
            Ok(()) => {
                *auth_state = AuthState::Authenticated { cookie_path: dest };
                state.command.message =
                    Some("Cookies imported — playback will use your account".into());
            }
            Err(e) => {
                state.command.message = Some(format!("Error: {}", e));
            }
        }
        return;
    }

    state.command.message = Some(format!("Unknown command: {}", cmd));
}

fn spawn_player_poll_task(socket_path: std::path::PathBuf, tx: ActionSender) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        let mut last_state = player::PlayerState::Stopped;

        loop {
            interval.tick().await;
            let path = socket_path.clone();
            let polled = tokio::task::spawn_blocking(move || poll_socket_state(&path)).await;
            let state = match polled {
                Ok(state) => state,
                Err(_) => player::PlayerState::Stopped,
            };

            if state != last_state {
                last_state = state.clone();
                if tx.send(Action::PlayerStateUpdate(state)).is_err() {
                    break;
                }
            }
        }
    });
}
