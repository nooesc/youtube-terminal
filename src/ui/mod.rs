pub mod card_grid;
pub mod now_playing;
pub mod search_bar;
pub mod tab_bar;
pub mod video_detail;
pub mod video_list;

use crate::app::AppState;
use crate::thumbnails::ThumbnailCache;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, thumb_cache: &ThumbnailCache) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search bar
            Constraint::Length(1), // tab bar
            Constraint::Min(3),    // main content
            Constraint::Length(3), // now-playing bar / command bar
        ])
        .split(f.area());

    search_bar::render(f, state, chunks[0]);
    tab_bar::render(f, state, chunks[1]);
    render_content(f, state, chunks[2], thumb_cache);

    if state.command.active || state.command.message.is_some() {
        render_command_bar(f, state, chunks[3]);
    } else {
        now_playing::render(f, state, chunks[3]);
    }
}

fn render_content(f: &mut Frame, state: &AppState, area: Rect, thumb_cache: &ThumbnailCache) {
    match &state.view {
        crate::app::View::Search => {
            video_list::render(f, state, area);
        }
        crate::app::View::Home => {
            card_grid::render(f, state, area, thumb_cache);
        }
        crate::app::View::VideoDetail(_) => {
            video_detail::render(f, state, area);
        }
        crate::app::View::ChannelDetail(id) => {
            let text = format!("Channel: {}", id);
            let content =
                Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Channel"));
            f.render_widget(content, area);
        }
    }
}

fn render_command_bar(f: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title("Command");
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 1 {
        return;
    }

    if state.command.active {
        // Show the command input with ":" prefix
        let text = format!(":{}", state.command.input);
        let cursor_x = inner.x + text.len() as u16;
        let paragraph = Paragraph::new(text).style(Style::default().fg(Color::White));
        f.render_widget(paragraph, inner);
        // Show cursor
        f.set_cursor_position((cursor_x.min(inner.right() - 1), inner.y));
    } else if let Some(msg) = &state.command.message {
        // Show status message
        let paragraph = Paragraph::new(msg.as_str()).style(Style::default().fg(Color::Yellow));
        f.render_widget(paragraph, inner);
    }
}
