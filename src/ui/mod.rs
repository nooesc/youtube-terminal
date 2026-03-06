pub mod now_playing;
pub mod search_bar;
pub mod tab_bar;
pub mod video_detail;
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
    now_playing::render(f, state, chunks[3]);
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
        crate::app::View::VideoDetail(_) => {
            video_detail::render(f, state, area);
        }
        crate::app::View::ChannelDetail(id) => {
            let text = format!("Channel: {}", id);
            let content = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title("Channel"));
            f.render_widget(content, area);
        }
    }
}

