use crate::app::AppState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    if state.loading.feed_loading {
        let loading = Paragraph::new("Loading subscriptions...")
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Subscriptions"),
            );
        f.render_widget(loading, area);
        return;
    }

    if state.subscription_channels.is_empty() {
        let empty =
            Paragraph::new("No subscriptions yet. Search for channels and press S to subscribe.")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Subscriptions"),
                );
        f.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = state
        .subscription_channels
        .iter()
        .enumerate()
        .map(|(i, channel)| {
            let marker = if i == state.cards.selected_row {
                "\u{25b8} "
            } else {
                "  "
            };
            let subs = channel
                .subscriber_count
                .map(format_count)
                .unwrap_or_default();
            let meta = if subs.is_empty() {
                String::new()
            } else {
                format!("{subs} subscribers")
            };
            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Green)),
                Span::styled(
                    &channel.name,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(meta, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.cards.selected_row));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Subscriptions"),
        )
        .highlight_style(Style::default().bg(Color::Rgb(98, 114, 98)));

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
