use crate::app::AppState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let (text, style) = if state.search.focused {
        let query = &state.search.query;
        let cursor_pos = state.search.cursor;
        let before = &query[..cursor_pos];
        let after = &query[cursor_pos..];
        let display = format!("/ {}|{}", before, after);
        (display, Style::default().fg(Color::Yellow))
    } else {
        ("/ Search...".to_string(), Style::default().fg(Color::DarkGray))
    };

    let search = Paragraph::new(text)
        .style(style)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(search, area);
}
