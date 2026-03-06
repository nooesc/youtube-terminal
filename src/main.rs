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
use models::{FeedItem, ItemType};
use player::mpv::MpvPlayer;
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

    // 3. Init auth state
    let mut auth_state = AuthState::load(&config);

    // 4. Init provider
    let mut provider = RustyPipeProvider::new(&config.rustypipe_storage_dir()).await?;
    if let AuthState::Authenticated { cookie_path } = &auth_state {
        if let Ok(content) = std::fs::read_to_string(cookie_path) {
            let _ = provider.set_cookies(&content).await;
            provider.set_authenticated(true);
        }
    }
    let provider = Arc::new(provider);

    // 5. Init player
    let mut player = MpvPlayer::new();

    // 6. Init app state
    let mut state = AppState::new();
    let mut thumb_cache = ThumbnailCache::new();

    // 7. Create action channels
    let (tx, mut rx) = create_action_channel();

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

        // Poll player state
        if player.is_running() {
            if let Ok(ps) = player.poll_state() {
                if ps != state.player_state {
                    state.dispatch(Action::PlayerStateUpdate(ps));
                }
            }
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
            execute_command(&cmd, state, config, auth_state, provider);
        }
        Action::Select => {
            // Determine what to load based on current view
            match &state.view {
                View::Search => {
                    if let Some(item) = state.selected_list_item().cloned() {
                        handle_item_select(&item, state, provider, tx);
                    }
                }
                View::Home => {
                    if let Some(item) = state.selected_card_item().cloned() {
                        handle_item_select(&item, state, provider, tx);
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
                                    state.previous_views.push(state.view.clone());
                                    state.view = View::ChannelDetail(channel_id);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
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
            if matches!(dir, Direction::Down) {
                check_load_more(state, provider, tx);
            }
        }
        Action::FeedLoaded(_, _) | Action::SearchResults(_, _) => {
            state.dispatch(action);
            spawn_thumbnail_downloads(state, tx, config, thumb_cache);
        }
        Action::AppendFeed(_, _) | Action::AppendSearch(_, _) => {
            state.dispatch(action);
            spawn_thumbnail_downloads(state, tx, config, thumb_cache);
        }
        Action::ThumbnailReady(ref key, ref path) => {
            // Load the downloaded image into the render cache
            // Use card grid thumbnail dimensions: width=24 (CARD_WIDTH-2), height=4 (THUMB_HEIGHT)
            let _ = thumb_cache.load(key, path, 24, 4);
            state.dispatch(action);
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

    let tx = tx.clone();
    let provider = Arc::clone(provider);
    let tab = state.tabs.active;

    tokio::spawn(async move {
        let result = match tab {
            Tab::ForYou => match provider.trending().await {
                Ok(page) => Some(LoadedPage::Trending(page)),
                Err(e) => {
                    let _ = tx.send(Action::ShowError(format!("Feed error: {}", e)));
                    None
                }
            },
            Tab::Subscriptions => match provider.subscription_feed(None).await {
                Ok(page) => Some(LoadedPage::SubscriptionFeed(page)),
                Err(e) => {
                    let _ = tx.send(Action::ShowError(format!("Subscriptions error: {}", e)));
                    None
                }
            },
            Tab::History => unreachable!("History tab handled synchronously above"),
        };

        if let Some(page) = result {
            let _ = tx.send(Action::FeedLoaded(req_id, Box::new(page)));
        }
    });
}

fn spawn_detail_load(
    state: &mut AppState,
    video_id: &str,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
) {
    state.loading.detail_request_id += 1;
    state.loading.detail_loading = true;
    let req_id = state.loading.detail_request_id;
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

fn check_load_more(state: &mut AppState, provider: &Arc<RustyPipeProvider>, tx: &ActionSender) {
    match &state.view {
        View::Home => {
            if state.loading.feed_loading || state.loading.loading_more_feed {
                return;
            }
            let total = state.cards.items.len();
            let idx = state.selected_card_index();
            let cols = state.cards.columns.max(1);
            // Trigger when within last 2 rows
            if total > 0 && idx >= total.saturating_sub(cols * 2) {
                if let Some(ctoken) = state.cards.continuation.clone() {
                    state.loading.loading_more_feed = true;
                    let req_id = state.loading.feed_request_id;
                    let tx = tx.clone();
                    let provider = Arc::clone(provider);
                    let tab = state.tabs.active;

                    tokio::spawn(async move {
                        let result = match tab {
                            Tab::Subscriptions => {
                                match provider.subscription_feed(Some(&ctoken)).await {
                                    Ok(page) => Some(LoadedPage::SubscriptionFeed(page)),
                                    Err(e) => {
                                        let _ = tx.send(Action::ShowError(format!(
                                            "Feed continuation error: {}",
                                            e
                                        )));
                                        None
                                    }
                                }
                            }
                            // Trending is not paginated; ForYou uses home_feed
                            Tab::ForYou => match provider.home_feed(Some(&ctoken)).await {
                                Ok(page) => Some(LoadedPage::Home(page)),
                                Err(e) => {
                                    let _ = tx.send(Action::ShowError(format!(
                                        "Feed continuation error: {}",
                                        e
                                    )));
                                    None
                                }
                            },
                            Tab::History => None,
                        };

                        if let Some(page) = result {
                            let _ = tx.send(Action::AppendFeed(req_id, Box::new(page)));
                        }
                    });
                }
            }
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
) {
    match item {
        FeedItem::Video(v) | FeedItem::Short(v) => {
            spawn_detail_load(state, &v.id, provider, tx);
        }
        FeedItem::Channel(c) => {
            state.previous_views.push(state.view.clone());
            state.view = View::ChannelDetail(c.id.clone());
        }
        FeedItem::Playlist(_) => {
            state.command.message = Some("Playlist view not yet implemented".into());
        }
    }
}

fn spawn_thumbnail_downloads(
    state: &mut AppState,
    tx: &ActionSender,
    config: &Config,
    thumb_cache: &ThumbnailCache,
) {
    let items = match &state.view {
        View::Home => &state.cards.items,
        View::Search => &state.video_list.items,
        _ => return,
    };

    let cache_dir = config.thumbnail_dir();
    for item in items.iter() {
        let key = item.thumbnail_key();
        if state.loading.thumbnail_loading.contains(&key) || thumb_cache.get(&key).is_some() {
            continue;
        }
        let url = item.thumbnail_url().to_string();
        if url.is_empty() {
            continue;
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
    provider: &Arc<RustyPipeProvider>,
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
                // Validate the imported cookies before reporting success
                if !auth::cookies::validate_cookies(&dest) {
                    state.command.message = Some("Error: imported cookies are invalid".into());
                    return;
                }
                // Reload cookies into the provider
                if let Ok(content) = std::fs::read_to_string(&dest) {
                    let provider = Arc::clone(provider);
                    tokio::spawn(async move {
                        let _ = provider.set_cookies(&content).await;
                        // Note: set_authenticated requires &mut, handled via auth_state below
                    });
                }
                // Update auth state
                *auth_state = AuthState::load(config);
                state.command.message = Some("Cookies imported successfully".into());
            }
            Err(e) => {
                state.command.message = Some(format!("Error: {}", e));
            }
        }
        return;
    }

    state.command.message = Some(format!("Unknown command: {}", cmd));
}
