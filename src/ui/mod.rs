pub mod tab_bar;

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

    render_search_bar(f, state, chunks[0]);
    tab_bar::render(f, state, chunks[1]);
    render_content(f, state, chunks[2]);
    render_now_playing(f, state, chunks[3]);
}

fn render_search_bar(f: &mut Frame, state: &AppState, area: Rect) {
    let text = if state.search.focused {
        format!("/ {}_", state.search.query)
    } else {
        "/ Search...".to_string()
    };

    let style = if state.search.focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let search = Paragraph::new(text)
        .style(style)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(search, area);
}

fn render_content(f: &mut Frame, state: &AppState, area: Rect) {
    // Placeholder -- will be replaced by card grid, video list, or detail view
    let text = match &state.view {
        crate::app::View::Home => {
            if state.loading.feed_loading {
                "Loading...".to_string()
            } else if state.cards.items.is_empty() {
                "No content. Press 1/2/3 to switch tabs.".to_string()
            } else {
                format!("{} items loaded", state.cards.items.len())
            }
        }
        crate::app::View::Search => {
            if state.loading.search_loading {
                "Searching...".to_string()
            } else if state.video_list.items.is_empty() {
                "Type a query and press Enter".to_string()
            } else {
                format!("{} results", state.video_list.items.len())
            }
        }
        crate::app::View::VideoDetail(id) => format!("Video detail: {}", id),
        crate::app::View::ChannelDetail(id) => format!("Channel detail: {}", id),
    };

    let content = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Content"));
    f.render_widget(content, area);
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
