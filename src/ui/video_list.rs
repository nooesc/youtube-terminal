use crate::app::AppState;
use crate::models::FeedItem;
use crate::ui::theme;
use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    if state.loading.search_loading {
        let loading = Paragraph::new("  Searching...")
            .style(Style::default().fg(theme::WARNING));
        f.render_widget(loading, area);
        return;
    }

    if state.video_list.items.is_empty() {
        let empty = Paragraph::new("  No results. Press / to search.")
            .style(Style::default().fg(theme::TEXT_DIM));
        f.render_widget(empty, area);
        return;
    }

    let total = state.video_list.items.len();
    let lines_per_item: u16 = 2;
    let visible = (area.height / lines_per_item) as usize;

    if visible == 0 {
        return;
    }

    let scroll_offset = if state.video_list.selected >= visible {
        state.video_list.selected - visible + 1
    } else {
        0
    };

    for i in 0..visible {
        let idx = i + scroll_offset;
        if idx >= total {
            break;
        }

        let item = &state.video_list.items[idx];
        let is_selected = idx == state.video_list.selected;
        let y = area.y + (i as u16) * lines_per_item;
        let w = area.width as usize;

        render_item(f, item, is_selected, area.x, y, area.width, w);
    }
}

fn render_item(
    f: &mut Frame,
    item: &FeedItem,
    selected: bool,
    x: u16,
    y: u16,
    width: u16,
    w: usize,
) {
    let (title, channel, meta, type_tag) = format_feed_item(item);

    let bg = if selected {
        theme::SELECTED_BG
    } else {
        Color::Reset
    };

    // Line 1: marker + title + optional type tag
    let marker = if selected { "\u{25b8} " } else { "  " };

    let mut line1_spans: Vec<Span> = vec![
        Span::styled(marker, Style::default().fg(theme::ACCENT).bg(bg)),
        Span::styled(
            truncate_str(&title, w.saturating_sub(2 + type_tag.len() + 1)),
            Style::default()
                .fg(theme::TEXT)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    if !type_tag.is_empty() {
        let title_display_len =
            2 + title
                .chars()
                .count()
                .min(w.saturating_sub(2 + type_tag.len() + 1));
        let padding = w.saturating_sub(title_display_len + type_tag.len());
        if padding > 0 {
            line1_spans.push(Span::styled(
                " ".repeat(padding),
                Style::default().bg(bg),
            ));
        }
        let tag_color = match item {
            FeedItem::Channel(_) => theme::CHANNEL,
            FeedItem::Playlist(_) => Color::Magenta,
            _ => theme::TEXT_DIM,
        };
        line1_spans.push(Span::styled(
            type_tag,
            Style::default().fg(tag_color).bg(bg),
        ));
    } else {
        let title_display_len = 2 + title.chars().count().min(w.saturating_sub(2));
        let padding = w.saturating_sub(title_display_len);
        if padding > 0 {
            line1_spans.push(Span::styled(
                " ".repeat(padding),
                Style::default().bg(bg),
            ));
        }
    }

    f.render_widget(
        Paragraph::new(Line::from(line1_spans)),
        Rect::new(x, y, width, 1),
    );

    // Line 2: indented channel + meta
    let mut line2_spans: Vec<Span> = vec![
        Span::styled("    ", Style::default().bg(bg)),
        Span::styled(
            channel.clone(),
            Style::default().fg(theme::CHANNEL).bg(bg),
        ),
    ];
    if !meta.is_empty() {
        line2_spans.push(Span::styled(
            "  \u{00b7}  ",
            Style::default().fg(theme::TEXT_DIM).bg(bg),
        ));
        line2_spans.push(Span::styled(
            meta.clone(),
            Style::default().fg(theme::TEXT_DIM).bg(bg),
        ));
    }

    let line2_content_len: usize = 4
        + channel.chars().count()
        + if meta.is_empty() {
            0
        } else {
            5 + meta.chars().count()
        };
    let line2_pad = w.saturating_sub(line2_content_len);
    if line2_pad > 0 {
        line2_spans.push(Span::styled(
            " ".repeat(line2_pad),
            Style::default().bg(bg),
        ));
    }

    if y + 1 < f.area().height {
        f.render_widget(
            Paragraph::new(Line::from(line2_spans)),
            Rect::new(x, y + 1, width, 1),
        );
    }
}

/// Returns (title, channel, meta_string, type_tag)
fn format_feed_item(item: &FeedItem) -> (String, String, String, String) {
    match item {
        FeedItem::Video(v) | FeedItem::Short(v) => {
            let meta = format_video_meta(v.view_count, v.duration.as_ref(), v.published);
            (v.title.clone(), v.channel.clone(), meta, String::new())
        }
        FeedItem::Channel(c) => {
            let subs = c.subscriber_count.map(format_count).unwrap_or_default();
            (
                c.name.clone(),
                "Channel".into(),
                format!("{} subs", subs),
                "[CH]".into(),
            )
        }
        FeedItem::Playlist(p) => {
            let count = p
                .video_count
                .map(|n| format!("{} videos", n))
                .unwrap_or_default();
            (
                p.title.clone(),
                p.channel.clone(),
                count,
                "[PL]".into(),
            )
        }
    }
}

fn format_video_meta(
    view_count: Option<u64>,
    duration: Option<&std::time::Duration>,
    published: Option<chrono::DateTime<chrono::Utc>>,
) -> String {
    let views = view_count
        .map(|n| format!("{} views", format_count(n)))
        .unwrap_or_default();
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
    parts.join("  \u{00b7}  ")
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

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max > 3 {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max).collect()
    }
}
