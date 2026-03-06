mod app;
mod auth;
mod config;
mod db;
mod event;
mod models;
mod player;
mod provider;
mod ui;

use app::{Action, AppState, LoadedPage, Tab, View};
use auth::AuthState;
use config::Config;
use db::Database;
use event::{create_action_channel, poll_event, ActionSender};
use models::FeedItem;
use player::mpv::MpvPlayer;
use player::PlayMode;
use provider::rustypipe_provider::RustyPipeProvider;
use provider::ContentProvider;

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
    let auth_state = AuthState::load(&config);

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

    // 7. Create action channels
    let (tx, mut rx) = create_action_channel();

    // 8. Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 9. Spawn initial feed load
    spawn_feed_load(&mut state, &provider, &tx);

    // 10. Main loop
    loop {
        // Render
        terminal.draw(|f| ui::render(f, &state))?;

        // Poll crossterm events
        if let Some(action) = poll_event(&state) {
            handle_action(
                action,
                &mut state,
                &mut player,
                &db,
                &config,
                &auth_state,
                &provider,
                &tx,
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
                &auth_state,
                &provider,
                &tx,
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

fn handle_action(
    action: Action,
    state: &mut AppState,
    player: &mut MpvPlayer,
    db: &Database,
    config: &Config,
    auth_state: &AuthState,
    provider: &Arc<RustyPipeProvider>,
    tx: &ActionSender,
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
                        eprintln!("Search error: {}", e);
                    }
                }
            });
        }
        Action::Select => {
            // Determine what to load based on current view
            match &state.view {
                View::Search => {
                    if let Some(item) = state.selected_list_item() {
                        if let Some(video_id) = get_video_id(item) {
                            spawn_detail_load(state, &video_id, provider, tx);
                        }
                    }
                }
                View::Home => {
                    if let Some(item) = state.selected_card_item() {
                        if let Some(video_id) = get_video_id(item) {
                            spawn_detail_load(state, &video_id, provider, tx);
                        }
                    }
                }
                View::VideoDetail(_) => {
                    if let Some(detail_state) = &state.detail {
                        let video_id = detail_state.detail.item.id.clone();
                        let cookie_path = auth_state.cookie_path();
                        match detail_state.selected_action {
                            0 => {
                                // Play Video
                                let _ = player.play(
                                    &format!("https://www.youtube.com/watch?v={}", video_id),
                                    PlayMode::Video,
                                    &config.mpv_geometry,
                                    config.mpv_ontop,
                                    cookie_path,
                                );
                                record_history(db, &detail_state.detail);
                            }
                            1 => {
                                // Play Audio Only
                                let _ = player.play(
                                    &format!("https://www.youtube.com/watch?v={}", video_id),
                                    PlayMode::AudioOnly,
                                    &config.mpv_geometry,
                                    config.mpv_ontop,
                                    cookie_path,
                                );
                                record_history(db, &detail_state.detail);
                            }
                            2 => {
                                // Open Channel -- navigate to channel detail
                                let channel_id =
                                    detail_state.detail.item.channel_id.clone();
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
            spawn_feed_load(state, provider, tx);
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
            let _ = player.play(
                &format!("https://www.youtube.com/watch?v={}", id),
                PlayMode::Video,
                &config.mpv_geometry,
                config.mpv_ontop,
                auth_state.cookie_path(),
            );
        }
        Action::PlayAudio(ref id) => {
            let _ = player.play(
                &format!("https://www.youtube.com/watch?v={}", id),
                PlayMode::AudioOnly,
                &config.mpv_geometry,
                config.mpv_ontop,
                auth_state.cookie_path(),
            );
        }
        _ => {
            // All other actions go through normal dispatch
            state.dispatch(action);
        }
    }
}

fn spawn_feed_load(state: &mut AppState, provider: &Arc<RustyPipeProvider>, tx: &ActionSender) {
    state.loading.feed_request_id += 1;
    state.loading.feed_loading = true;
    let req_id = state.loading.feed_request_id;
    let tx = tx.clone();
    let provider = Arc::clone(provider);
    let tab = state.tabs.active;

    tokio::spawn(async move {
        let result = match tab {
            Tab::ForYou => match provider.trending().await {
                Ok(page) => Some(LoadedPage::Trending(page)),
                Err(e) => {
                    eprintln!("Feed error: {}", e);
                    None
                }
            },
            Tab::Subscriptions => match provider.subscription_feed(None).await {
                Ok(page) => Some(LoadedPage::SubscriptionFeed(page)),
                Err(e) => {
                    eprintln!("Subscriptions error: {}", e);
                    None
                }
            },
            Tab::History => {
                // History is loaded from local DB, not from provider.
                // Will be handled in Task 20.
                None
            }
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
                eprintln!("Detail error: {}", e);
            }
        }
    });
}

fn get_video_id(item: &FeedItem) -> Option<String> {
    match item {
        FeedItem::Video(v) | FeedItem::Short(v) => Some(v.id.clone()),
        _ => None,
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
