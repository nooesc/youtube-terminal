use crate::app::AppState;
use crate::models::FeedItem;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    if state.loading.search_loading {
        let loading = Paragraph::new("Searching...")
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Search Results"),
            );
        f.render_widget(loading, area);
        return;
    }

    if state.video_list.items.is_empty() {
        let empty = Paragraph::new("No results. Press / to search.")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Search Results"),
            );
        f.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = state
        .video_list
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let (title, channel, meta) = format_feed_item(item);
            let marker = if i == state.video_list.selected {
                "▸ "
            } else {
                "  "
            };
            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    title,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" — "),
                Span::styled(channel, Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(meta, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.video_list.selected));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search Results"),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, area, &mut list_state);
}

fn format_feed_item(item: &FeedItem) -> (String, String, String) {
    match item {
        FeedItem::Video(v) | FeedItem::Short(v) => {
            let meta = format_video_meta(v.view_count, v.duration.as_ref());
            (v.title.clone(), v.channel.clone(), meta)
        }
        FeedItem::Channel(c) => {
            let subs = c.subscriber_count.map(format_count).unwrap_or_default();
            (c.name.clone(), "Channel".into(), format!("{} subs", subs))
        }
        FeedItem::Playlist(p) => {
            let count = p
                .video_count
                .map(|n| format!("{} videos", n))
                .unwrap_or_default();
            (p.title.clone(), p.channel.clone(), count)
        }
    }
}

fn format_video_meta(view_count: Option<u64>, duration: Option<&std::time::Duration>) -> String {
    let views = view_count.map(format_count).unwrap_or_default();
    let dur = duration
        .map(|d| {
            let secs = d.as_secs();
            let m = secs / 60;
            let s = secs % 60;
            if m >= 60 {
                format!("{}:{:02}:{:02}", m / 60, m % 60, s)
            } else {
                format!("{}:{:02}", m, s)
            }
        })
        .unwrap_or_default();

    match (views.is_empty(), dur.is_empty()) {
        (false, false) => format!("{} · {}", views, dur),
        (false, true) => views,
        (true, false) => dur,
        (true, true) => String::new(),
    }
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
