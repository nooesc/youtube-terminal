use crate::app::AppState;
use crate::ui::theme;
use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, area: Rect, channel_id: &str) {
    let detail_state = match &state.channel_detail {
        Some(detail) if detail.detail.item.id == channel_id => detail,
        _ => {
            let loading = Paragraph::new("  Loading channel details...")
                .style(Style::default().fg(theme::WARNING));
            f.render_widget(loading, area);
            return;
        }
    };

    let detail = &detail_state.detail;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // header
            Constraint::Length(3), // subscribe action
            Constraint::Min(5),   // video list
        ])
        .split(area);

    // Header
    let subs = detail
        .item
        .subscriber_count
        .map(format_count)
        .unwrap_or_else(|| "hidden".to_string());
    let videos_count = detail
        .video_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let mut header_lines = vec![
        Line::from(vec![
            Span::styled("\u{2190} ", Style::default().fg(theme::TEXT_DIM)),
            Span::styled(
                &detail.item.name,
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            format!("{subs} subscribers \u{00b7} {videos_count} videos"),
            Style::default().fg(theme::CHANNEL),
        )),
    ];
    if !detail.description.is_empty() {
        let desc_preview: String = detail.description.chars().take(120).collect();
        let suffix = if detail.description.chars().count() > 120 {
            "..."
        } else {
            ""
        };
        header_lines.push(Line::from(Span::styled(
            format!("{desc_preview}{suffix}"),
            Style::default().fg(theme::TEXT_DIM),
        )));
    }

    let header = Paragraph::new(header_lines).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme::BORDER)),
    );
    f.render_widget(header, chunks[0]);

    // Subscribe action
    let sub_label = if detail_state.is_subscribed {
        "\u{2605} Unsubscribe"
    } else {
        "\u{2606} Subscribe"
    };
    let sub_selected = detail_state.selected_action == 0;
    let sub_style = if sub_selected {
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::TEXT)
    };
    let marker = if sub_selected { "\u{25b8} " } else { "  " };
    let sub_paragraph = Paragraph::new(Line::from(vec![
        Span::styled(marker, Style::default().fg(theme::ACCENT)),
        Span::styled(sub_label, sub_style),
    ]));
    f.render_widget(sub_paragraph, chunks[1]);

    // Video list
    if detail.videos.is_empty() {
        let empty = Paragraph::new("  No videos found")
            .style(Style::default().fg(theme::TEXT_DIM));
        f.render_widget(empty, chunks[2]);
        return;
    }

    let in_videos = detail_state.selected_action >= 1;
    let items: Vec<ListItem> = detail
        .videos
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let selected = in_videos && i == detail_state.selected_video;
            let marker = if selected { "\u{25b8} " } else { "  " };
            let title_style = if selected {
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::TEXT)
            };
            let meta = format_video_meta(v.view_count, v.duration.as_ref(), v.published);
            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(theme::ACCENT)),
                Span::styled(&v.title, title_style),
                Span::raw("  "),
                Span::styled(meta, Style::default().fg(theme::TEXT_DIM)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default();
    if in_videos {
        list_state.select(Some(detail_state.selected_video));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(theme::BORDER)),
        )
        .highlight_style(Style::default().bg(theme::SELECTED_BG));

    f.render_stateful_widget(list, chunks[2], &mut list_state);
}

fn format_video_meta(
    view_count: Option<u64>,
    duration: Option<&std::time::Duration>,
    published: Option<chrono::DateTime<chrono::Utc>>,
) -> String {
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
    let time_ago = published.map(format_time_ago).unwrap_or_default();
    let mut parts: Vec<&str> = Vec::new();
    if !views.is_empty() {
        parts.push(&views);
    }
    if !dur.is_empty() {
        parts.push(&dur);
    }
    if !time_ago.is_empty() {
        parts.push(&time_ago);
    }
    parts.join(" \u{00b7} ")
}

fn format_time_ago(dt: chrono::DateTime<chrono::Utc>) -> String {
    let ago = Utc::now().signed_duration_since(dt);
    if ago.num_minutes() < 1 {
        "just now".to_string()
    } else if ago.num_hours() < 1 {
        format!("{}m ago", ago.num_minutes())
    } else if ago.num_hours() < 24 {
        format!("{}h ago", ago.num_hours())
    } else if ago.num_days() < 30 {
        format!("{}d ago", ago.num_days())
    } else if ago.num_days() < 365 {
        format!("{}mo ago", ago.num_days() / 30)
    } else {
        format!("{}y ago", ago.num_days() / 365)
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
