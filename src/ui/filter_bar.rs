use crate::app::AppState;
use crate::ui::theme;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let filter = &state.search.filter;

    let filters: [(&str, &str, usize); 4] = [
        ("Sort", filter.sort.label(), 0),
        ("Date", filter.date.label(), 1),
        ("Type", filter.item_type.label(), 2),
        ("Length", filter.length.label(), 3),
    ];

    let mut spans: Vec<Span> = Vec::new();

    if filter.active {
        spans.push(Span::styled(
            " [f] exit [r] reset  ",
            Style::default().fg(theme::TEXT_DIM),
        ));
    } else {
        spans.push(Span::styled(
            " [f] filter ",
            Style::default().fg(theme::TEXT_DIM),
        ));
    }

    for (i, (label, value, idx)) in filters.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " \u{2502} ",
                Style::default().fg(theme::BORDER),
            ));
        }

        let is_focused = filter.active && filter.focused_index == *idx;
        let is_non_default = match idx {
            0 => filter.sort != crate::app::SearchSort::Relevance,
            1 => filter.date != crate::app::SearchDate::Any,
            2 => filter.item_type != crate::app::SearchItemType::All,
            3 => filter.length != crate::app::SearchLength::Any,
            _ => false,
        };

        let label_style = if is_focused {
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::TEXT_DIM)
        };

        let value_style = if is_focused {
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else if is_non_default {
            Style::default().fg(theme::WARNING)
        } else {
            Style::default().fg(theme::TEXT)
        };

        spans.push(Span::styled(format!("{}: ", label), label_style));
        spans.push(Span::styled(*value, value_style));

        if is_focused {
            spans.push(Span::styled(
                " \u{25b2}\u{25bc}",
                Style::default().fg(theme::ACCENT),
            ));
        }
    }

    let line = Line::from(spans);
    let bar = Paragraph::new(line);
    f.render_widget(bar, area);
}
