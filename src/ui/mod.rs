pub mod card_grid;
pub mod channel_detail;
pub mod filter_bar;
pub mod now_playing;
pub mod playlist_detail;
pub mod popup;
pub mod saved_searches_list;
pub mod search_bar;
pub mod subscription_list;
pub mod tab_bar;
pub mod video_detail;
pub mod theme;
pub mod video_list;

use crate::app::AppState;
use crate::thumbnails::ThumbnailCache;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, thumb_cache: &ThumbnailCache) {
    let show_filter_bar = matches!(state.view, crate::app::View::Search);

    let chunks = if show_filter_bar {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // search bar
                Constraint::Length(1), // tab bar
                Constraint::Length(1), // filter bar
                Constraint::Min(3),   // main content
                Constraint::Length(3), // now-playing bar / command bar
            ])
            .split(f.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // search bar
                Constraint::Length(1), // tab bar
                Constraint::Length(0), // no filter bar
                Constraint::Min(3),   // main content
                Constraint::Length(3), // now-playing bar / command bar
            ])
            .split(f.area())
    };

    search_bar::render(f, state, chunks[0]);
    tab_bar::render(f, state, chunks[1]);

    if show_filter_bar {
        filter_bar::render(f, state, chunks[2]);
    }

    render_content(f, state, chunks[3], thumb_cache);

    if state.command.active || state.command.message.is_some() {
        render_command_bar(f, state, chunks[4]);
    } else {
        now_playing::render(f, state, chunks[4]);
    }

    // Popup overlay (must render last, on top of everything)
    if state.popup.is_some() {
        popup::render(f, state);
    }
}

fn render_content(f: &mut Frame, state: &AppState, area: Rect, thumb_cache: &ThumbnailCache) {
    match &state.view {
        crate::app::View::Search => {
            video_list::render(f, state, area);
        }
        crate::app::View::Home => {
            if state.tabs.active == crate::app::Tab::SavedSearches {
                saved_searches_list::render(f, state, area);
            } else if state.tabs.active == crate::app::Tab::Subscriptions {
                subscription_list::render(f, state, area, thumb_cache);
            } else {
                card_grid::render(f, state, area, thumb_cache);
            }
        }
        crate::app::View::VideoDetail(_) => {
            video_detail::render(f, state, area, thumb_cache);
        }
        crate::app::View::ChannelDetail(id) => {
            channel_detail::render(f, state, area, id);
        }
        crate::app::View::PlaylistDetail(_) => {
            playlist_detail::render(f, state, area);
        }
    }
}

fn render_command_bar(f: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme::BORDER))
        .title("Command");
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 1 {
        return;
    }

    if state.command.active {
        // Show the command input with ":" prefix
        let text = format!(":{}", state.command.input);
        let cursor_x = inner.x + text.len() as u16;
        let paragraph = Paragraph::new(text).style(Style::default().fg(theme::TEXT));
        f.render_widget(paragraph, inner);
        // Show cursor
        f.set_cursor_position((cursor_x.min(inner.right() - 1), inner.y));
    } else if let Some(msg) = &state.command.message {
        // Show status message
        let paragraph = Paragraph::new(msg.as_str()).style(Style::default().fg(theme::WARNING));
        f.render_widget(paragraph, inner);
    }
}
