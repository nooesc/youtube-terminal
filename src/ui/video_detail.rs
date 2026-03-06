use crate::app::AppState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let detail_state = match &state.detail {
        Some(d) => d,
        None => {
            if state.loading.detail_loading {
                let loading = Paragraph::new("Loading video details...")
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL).title("Video Detail"));
                f.render_widget(loading, area);
            }
            return;
        }
    };

    let detail = &detail_state.detail;

    // Split into: header, description, actions
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // header (title, channel, stats)
            Constraint::Min(5),    // description
            Constraint::Length(7), // action menu
        ])
        .split(area);

    // Header
    render_header(f, detail, chunks[0]);

    // Description
    render_description(f, &detail.description, chunks[1]);

    // Action menu
    render_actions(f, detail_state.selected_action, chunks[2]);
}

fn render_header(f: &mut Frame, detail: &crate::models::VideoDetail, area: Rect) {
    let views = detail.item.view_count.map(format_count).unwrap_or_default();

    let likes = detail.like_count.map(format_count).unwrap_or_default();

    let stats = if !likes.is_empty() {
        format!("{} views · {} likes", views, likes)
    } else {
        format!("{} views", views)
    };

    let text = vec![
        Line::from(Span::styled(
            &detail.item.title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            &detail.item.channel,
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::styled(stats, Style::default().fg(Color::DarkGray))),
    ];

    let header = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("\u{2190} Back (ESC)"),
    );
    f.render_widget(header, area);
}

fn render_description(f: &mut Frame, description: &str, area: Rect) {
    let desc = Paragraph::new(description)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Description"));
    f.render_widget(desc, area);
}

fn render_actions(f: &mut Frame, selected: usize, area: Rect) {
    let actions = ["Play Video (mpv window)", "Play Audio Only", "Open Channel"];

    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let marker = if i == selected { "\u{25b8} " } else { "  " };
            let style = if i == selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(*action, style),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(selected));

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Actions"));

    f.render_stateful_widget(list, area, &mut list_state);
}

fn format_count(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
