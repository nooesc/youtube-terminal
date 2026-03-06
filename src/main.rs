mod app;
mod auth;
mod config;
mod db;
mod event;
mod models;
mod player;
mod provider;
mod thumbnails;
mod ui;

use app::{Action, AppState, Direction, LoadedPage, Tab, View};
use auth::AuthState;
use config::Config;
use db::Database;
use event::{create_action_channel, poll_event, ActionSender};
use models::{FeedItem, ItemType, VideoItem};
use player::mpv::{poll_socket_state, MpvPlayer};
use player::PlayMode;
use provider::rustypipe_provider::RustyPipeProvider;
use provider::ContentProvider;
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

    // 2. Init database
    let db = Database::open(&config.db_path())?;
    for path in db.cleanup_old_thumbnails(30, 1000).unwrap_or_default() {
        let _ = std::fs::remove_file(path);
    }

    // 3. Init auth state
    let mut auth_state = AuthState::load(&config);

    // 4. Init provider
    // Note: cookies are NOT loaded into RustyPipe because its set_cookie_txt
    // validates by hitting YouTube servers, which fails with rotated sessions.
    // Cookies are only used for mpv/yt-dlp playback via the cookie file path.
    let provider = RustyPipeProvider::new(&config.rustypipe_storage_dir()).await?;
    let provider = Arc::new(provider);

    // 5. Init player
    let mut player = MpvPlayer::new();

    // 6. Init app state
    let mut state = AppState::new();
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

    // 9. Spawn initial feed load
    spawn_feed_load(&mut state, &provider, &tx, &db);

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
    player.stop();
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
            // Spawn search task
            let tx = tx.clone();
            let provider = Arc::clone(provider);
            let req_id = state.loading.search_request_id;
            tokio::spawn(async move {
                match provider.search(&query, None).await {
                    Ok(page) => {
                        let _ = tx.send(Action::SearchResults(req_id, page));
                    }
                    Err(e) => {
                        let _ = tx.send(Action::ShowError(format!("Search error: {}", e)));
                    }
                }
            });
        }
        Action::SubmitCommand(ref cmd) => {
            let cmd = cmd.trim().to_string();
            state.dispatch(action);
            execute_command(&cmd, state, config, auth_state);
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
                    if state.tabs.active == Tab::Subscriptions {
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
                    if let Some(detail_state) = &state.detail {
                        let video_id = detail_state.detail.item.id.clone();
                        let cookie_path = auth_state.cookie_path();
                        match detail_state.selected_action {
                            0 => {
                                // Play Video
                                if let Err(e) = player.play(
                                    &format!("https://www.youtube.com/watch?v={}", video_id),
                                    PlayMode::Video,
                                    &config.mpv_geometry,
                                    config.mpv_ontop,
                                    cookie_path,
                                ) {
                                    state.command.message =
                                        Some(format!("Playback error: {} (is mpv installed?)", e));
                                } else {
                                    record_history(db, &detail_state.detail);
                                }
                            }
                            1 => {
                                // Play Audio Only
                                if let Err(e) = player.play(
                                    &format!("https://www.youtube.com/watch?v={}", video_id),
                                    PlayMode::AudioOnly,
                                    &config.mpv_geometry,
                                    config.mpv_ontop,
                                    cookie_path,
                                ) {
                                    state.command.message =
                                        Some(format!("Playback error: {} (is mpv installed?)", e));
                                } else {
                                    record_history(db, &detail_state.detail);
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
                    if let Some(detail_state) = &state.playlist_detail {
                        let playlist_id = detail_state.detail.item.id.clone();
                        let cookie_path = auth_state.cookie_path();
                        match detail_state.selected_action {
                            0 => {
                                if let Err(e) = player.play(
                                    &format!(
                                        "https://www.youtube.com/playlist?list={}",
                                        playlist_id
                                    ),
                                    PlayMode::Video,
                                    &config.mpv_geometry,
                                    config.mpv_ontop,
                                    cookie_path,
                                ) {
                                    state.command.message =
                                        Some(format!("Playback error: {} (is mpv installed?)", e));
                                }
                            }
                            1 => {
                                if let Err(e) = player.play(
                                    &format!(
                                        "https://www.youtube.com/playlist?list={}",
                                        playlist_id
                                    ),
                                    PlayMode::AudioOnly,
                                    &config.mpv_geometry,
                                    config.mpv_ontop,
                                    cookie_path,
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
        Action::SwitchTab(_) => {
            state.dispatch(action);
            spawn_feed_load(state, provider, tx, db);
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
        Action::PlayVideo(ref id) => {
            if let Err(e) = player.play(
                &format!("https://www.youtube.com/watch?v={}", id),
                PlayMode::Video,
                &config.mpv_geometry,
                config.mpv_ontop,
                auth_state.cookie_path(),
            ) {
                state.command.message = Some(format!("Playback error: {} (is mpv installed?)", e));
            }
        }
        Action::PlayAudio(ref id) => {
            if let Err(e) = player.play(
                &format!("https://www.youtube.com/watch?v={}", id),
                PlayMode::AudioOnly,
                &config.mpv_geometry,
                config.mpv_ontop,
                auth_state.cookie_path(),
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
            state.dispatch(action);
        }
        Action::ChannelDetailLoaded(_, ref detail) => {
            let is_subbed = db.is_subscribed(&detail.item.id).unwrap_or(false);
            state.dispatch(action);
            if let Some(ref mut cd) = state.channel_detail {
                cd.is_subscribed = is_subbed;
            }
        }
        Action::PlaylistDetailLoaded(_, _) => {
            state.dispatch(action);
        }
        Action::ThumbnailReady(ref key, ref path) => {
            // Load the downloaded image into the render cache
            // Use card grid thumbnail dimensions: width=24 (CARD_WIDTH-2), height=4 (THUMB_HEIGHT)
            if thumb_cache.load(key, path, 24, 4).is_ok() {
                let _ = db.set_thumbnail_path(key, path);
                state.dispatch(action);
            } else {
                let _ = std::fs::remove_file(path);
                state.dispatch(Action::ThumbnailFailed(key.clone()));
            }
        }
        Action::ThumbnailFailed(_) => {
            state.dispatch(action);
        }
        Action::Subscribe(ref channel) => {
            match db.subscribe(channel) {
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
            }
        }
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
                        state.command.message =
                            Some(format!("Unsubscribed from {}", channel.name));
                    }
                } else if db.subscribe(&channel).is_ok() {
                    state.command.message = Some(format!("Subscribed to {}", channel.name));
                }
            } else {
                state.command.message = Some("Select a channel to subscribe".into());
            }
        }
        _ => {
            // All other actions go through normal dispatch
            state.dispatch(action);
        }
    }
}

fn spawn_feed_load(
    state: &mut AppState,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
    db: &Database,
) {
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
                items: all_videos.into_iter().map(models::FeedItem::Video).collect(),
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

        if let Ok(Some(existing_path)) = db.get_thumbnail_path(&key) {
            if existing_path.exists() {
                let _ = tx.send(Action::ThumbnailReady(key.clone(), existing_path));
                continue;
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
            continue;
        }

        state.loading.thumbnail_loading.insert(key.clone());
        let tx = tx.clone();
        let key_clone = key.clone();
        let cache_dir = cache_dir.clone();
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
    config: &Config,
    auth_state: &mut AuthState,
) {
    if cmd == "q" || cmd == "quit" {
        state.should_quit = true;
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
        let mut interval = tokio::time::interval(Duration::from_millis(250));
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
