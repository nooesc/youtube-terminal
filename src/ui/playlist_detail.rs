use crate::app::AppState;
use crate::ui::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let detail_state = match &state.playlist_detail {
        Some(detail) => detail,
        None => {
            let loading = Paragraph::new("  Loading playlist details...")
                .style(Style::default().fg(theme::WARNING));
            f.render_widget(loading, area);
            return;
        }
    };

    let detail = &detail_state.detail;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(7),
        ])
        .split(area);

    let count = detail
        .item
        .video_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("\u{2190} ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled(
                &detail.item.title,
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            format!("{} \u{00b7} {} videos", detail.item.channel, count),
            Style::default().fg(theme::CHANNEL),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme::BORDER)),
    );
    f.render_widget(header, chunks[0]);

    let description = if detail.description.is_empty() {
        "No description available."
    } else {
        detail.description.as_str()
    };
    let body = Paragraph::new(description)
        .style(Style::default().fg(theme::TEXT))
        .wrap(Wrap { trim: true });
    f.render_widget(body, chunks[1]);

    let actions = [
        "Play Playlist (mpv window)",
        "Play Audio Only",
        "Open Channel",
    ];
    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let marker = if i == detail_state.selected_action {
                "\u{25b8} "
            } else {
                "  "
            };
            let style = if i == detail_state.selected_action {
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::TEXT)
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(theme::ACCENT)),
                Span::styled(*action, style),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(detail_state.selected_action));
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme::BORDER)),
    );
    f.render_stateful_widget(list, chunks[2], &mut list_state);
}
