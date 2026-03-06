mod app;
mod auth;
mod config;
mod db;
mod event;
mod models;
mod player;
mod provider;

use app::{AppState, Tab, View};
use auth::AuthState;
use config::Config;
use db::Database;
use event::{create_action_channel, poll_event};
use player::mpv::MpvPlayer;
use provider::rustypipe_provider::RustyPipeProvider;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::io;

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
    let _db = Database::open(&config.db_path())?;

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

    // 5. Init player
    let mut _player = MpvPlayer::new();

    // 6. Init app state
    let mut state = AppState::new();

    // 7. Create action channels
    let (_tx, mut rx) = create_action_channel();

    // 8. Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 9. Main loop
    loop {
        // Render
        terminal.draw(|f| render_placeholder(f, &state))?;

        // Poll crossterm events
        if let Some(action) = poll_event(&state) {
            state.dispatch(action);
        }

        // Drain async actions from channel
        while let Ok(action) = rx.try_recv() {
            state.dispatch(action);
        }

        if state.should_quit {
            break;
        }
    }

    // 10. Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

fn render_placeholder(f: &mut Frame, state: &AppState) {
    let area = f.area();

    let tab_name = match state.tabs.active {
        Tab::ForYou => "For You",
        Tab::Subscriptions => "Subscriptions",
        Tab::History => "History",
    };

    let view_name = match &state.view {
        View::Home => "Home".to_string(),
        View::Search => format!("Search: {}", state.search.query),
        View::VideoDetail(id) => format!("Video: {}", id),
        View::ChannelDetail(id) => format!("Channel: {}", id),
    };

    let text = format!(
        "youtube-terminal v0.1.0\n\nTab: {} | View: {}\n\n\
        Keys: q=quit, /=search, 1/2/3=tabs, hjkl=navigate, Enter=select, Esc=back\n\
        Playback: Space=pause, </>=seek, +/-=volume",
        tab_name, view_name
    );

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("youtube-terminal"));
    f.render_widget(paragraph, area);
}
