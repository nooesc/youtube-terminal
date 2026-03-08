use crate::app::AppState;
use crate::ui::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(theme::BORDER));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(" /  ", Style::default().fg(theme::TEXT_DIM)));

    if state.search.focused {
        let query = &state.search.query;
        let cursor_pos = state.search.cursor;
        let before = &query[..cursor_pos];
        let after = &query[cursor_pos..];
        spans.push(Span::styled(before, Style::default().fg(theme::TEXT)));
        spans.push(Span::styled("\u{2502}", Style::default().fg(theme::ACCENT)));
        spans.push(Span::styled(after, Style::default().fg(theme::TEXT)));
    } else if !state.search.query.is_empty() {
        spans.push(Span::styled(
            state.search.query.as_str(),
            Style::default().fg(theme::TEXT),
        ));
    } else {
        spans.push(Span::styled(
            "Search...",
            Style::default().fg(theme::TEXT_DIM),
        ));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), inner);

    // Show "S save" hint right-aligned when viewing search results
    let show_save_hint = !state.search.focused
        && !state.search.query.is_empty()
        && state.view == crate::app::View::Search
        && state.popup.is_none();

    if show_save_hint {
        let hint = Line::from(vec![
            Span::styled("S", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(" save", Style::default().fg(theme::TEXT_DIM)),
        ]);
        let hint_width = 6u16;
        let hint_x = inner.right().saturating_sub(hint_width + 1);
        if hint_x > inner.x + 10 {
            f.render_widget(Paragraph::new(hint), Rect::new(hint_x, inner.y, hint_width, 1));
        }
    }
}
