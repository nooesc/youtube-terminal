use crate::app::AppState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let detail_state = match &state.playlist_detail {
        Some(detail) => detail,
        None => {
            let loading = Paragraph::new("Loading playlist details...")
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Playlist"));
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
        Line::from(Span::styled(
            &detail.item.title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{} · {} videos", detail.item.channel, count),
            Style::default().fg(Color::Cyan),
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("Playlist"));
    f.render_widget(header, chunks[0]);

    let description = if detail.description.is_empty() {
        "No description available."
    } else {
        detail.description.as_str()
    };
    let body = Paragraph::new(description)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Description"));
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
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(*action, Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(detail_state.selected_action));
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Actions"));
    f.render_stateful_widget(list, chunks[2], &mut list_state);
}
