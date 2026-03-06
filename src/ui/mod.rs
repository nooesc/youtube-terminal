pub mod search_bar;
pub mod tab_bar;
pub mod video_list;

use crate::app::AppState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // search bar
            Constraint::Length(1),  // tab bar
            Constraint::Min(3),    // main content
            Constraint::Length(3), // now-playing bar
        ])
        .split(f.area());

    search_bar::render(f, state, chunks[0]);
    tab_bar::render(f, state, chunks[1]);
    render_content(f, state, chunks[2]);
    render_now_playing(f, state, chunks[3]);
}

fn render_content(f: &mut Frame, state: &AppState, area: Rect) {
    match &state.view {
        crate::app::View::Search => {
            video_list::render(f, state, area);
        }
        crate::app::View::Home => {
            // Placeholder for card grid (Task 17)
            let text = if state.loading.feed_loading {
                "Loading...".to_string()
            } else if state.cards.items.is_empty() {
                "No content. Press / to search.".to_string()
            } else {
                format!("{} items loaded", state.cards.items.len())
            };
            let content = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Content"));
            f.render_widget(content, area);
        }
        crate::app::View::VideoDetail(id) => {
            // Placeholder for video detail (Task 14)
            let text = format!("Video detail: {}", id);
            let content = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Video Detail"));
            f.render_widget(content, area);
        }
        crate::app::View::ChannelDetail(id) => {
            let text = format!("Channel: {}", id);
            let content = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Channel"));
            f.render_widget(content, area);
        }
    }
}

fn render_now_playing(f: &mut Frame, state: &AppState, area: Rect) {
    use crate::player::PlayerState;

    let text = match &state.player_state {
        PlayerState::Stopped => "No media playing".to_string(),
        PlayerState::Playing(info) => {
            format!(
                "▶ {} — {}:{:02}/{}:{:02}",
                info.title,
                (info.time_pos / 60.0) as u32,
                (info.time_pos % 60.0) as u32,
                (info.duration / 60.0) as u32,
                (info.duration % 60.0) as u32,
            )
        }
        PlayerState::Paused(info) => {
            format!(
                "❚❚ {} — {}:{:02}/{}:{:02}",
                info.title,
                (info.time_pos / 60.0) as u32,
                (info.time_pos % 60.0) as u32,
                (info.duration / 60.0) as u32,
                (info.duration % 60.0) as u32,
            )
        }
    };

    let now_playing = Paragraph::new(text)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title("Now Playing"));
    f.render_widget(now_playing, area);
}
