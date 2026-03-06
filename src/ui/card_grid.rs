use crate::app::AppState;
use crate::models::FeedItem;
use crate::thumbnails::ThumbnailCache;
use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub const CARD_WIDTH: u16 = 50;
pub const CARD_HEIGHT: u16 = 14;
pub const THUMB_HEIGHT: u16 = 8;

pub fn render(f: &mut Frame, state: &AppState, area: Rect, thumb_cache: &ThumbnailCache) {
    if state.loading.feed_loading && state.cards.items.is_empty() {
        let loading = Paragraph::new("Loading...").style(Style::default().fg(Color::Yellow));
        f.render_widget(
            loading,
            Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1),
        );
        return;
    }

    if state.cards.items.is_empty() {
        let msg = if state.tabs.active == crate::app::Tab::ForYou {
            "Subscribe to channels to see their videos here. Press / to search."
        } else {
            "No content yet. Press / to search."
        };
        let empty = Paragraph::new(msg).style(Style::default().fg(Color::DarkGray));
        f.render_widget(
            empty,
            Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), 1),
        );
        return;
    }

    // Use full area — no outer border to maximize space
    let inner = area;

    // Calculate grid dimensions
    let cols = ((inner.width + 1) / (CARD_WIDTH + 1)).max(1) as usize;
    let total = state.cards.items.len();
    let rows_count = total.div_ceil(cols);

    // Calculate visible rows based on available height
    let visible_rows = (inner.height / CARD_HEIGHT) as usize;
    if visible_rows == 0 {
        return;
    }

    // Scroll offset -- keep selected row visible
    let scroll_offset = if state.cards.selected_row >= visible_rows {
        state.cards.selected_row - visible_rows + 1
    } else {
        0
    };

    // Render visible cards
    for row_idx in 0..visible_rows.min(rows_count) {
        let actual_row = row_idx + scroll_offset;
        if actual_row >= rows_count {
            break;
        }

        for col in 0..cols {
            let item_idx = actual_row * cols + col;
            if item_idx >= total {
                break;
            }

            let x = inner.x + (col as u16) * (CARD_WIDTH + 1);
            let y = inner.y + (row_idx as u16) * CARD_HEIGHT;

            if x + CARD_WIDTH > inner.x + inner.width || y + CARD_HEIGHT > inner.y + inner.height {
                break;
            }

            let card_area = Rect::new(x, y, CARD_WIDTH, CARD_HEIGHT);
            let is_selected =
                actual_row == state.cards.selected_row && col == state.cards.selected_col;

            render_card(
                f,
                &state.cards.items[item_idx],
                card_area,
                is_selected,
                thumb_cache,
            );
        }
    }
}

fn render_card(
    f: &mut Frame,
    item: &FeedItem,
    area: Rect,
    selected: bool,
    thumb_cache: &ThumbnailCache,
) {
    let selected_green_bg = Color::Rgb(98, 114, 98);
    let (border_style, bg) = if selected {
        (Style::default().fg(Color::Green), selected_green_bg)
    } else {
        (Style::default().fg(Color::DarkGray), Color::Reset)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(bg));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    let (title, channel, views, time_ago) = format_card_item(item);
    let w = inner.width as usize;

    // Build text lines (title up to 2 lines, channel, meta) — anchored to bottom
    let title_chars: Vec<char> = title.chars().collect();
    let title_lines: Vec<String> = if title_chars.len() > w && w > 3 {
        let line1: String = title_chars[..w].iter().collect();
        let rest: String = title_chars[w..].iter().collect();
        vec![line1, truncate_str(&rest, w)]
    } else {
        vec![title.clone()]
    };

    let meta = if time_ago.is_empty() {
        views
    } else if views.is_empty() {
        time_ago
    } else {
        format!("{} \u{00b7} {}", views, time_ago)
    };

    // text_lines = title(1-2) + channel(1) + meta(1) = 3-4 lines
    let text_count = title_lines.len() as u16 + 2; // +1 channel +1 meta
    let text_start_y = inner.y + inner.height - text_count;

    // Thumbnail fills everything above the text
    let thumb_h = inner.height.saturating_sub(text_count);
    if thumb_h > 0 {
        let thumb_area = Rect::new(inner.x, inner.y, inner.width, thumb_h);
        render_thumbnail(f, item, thumb_area, thumb_cache);
    }

    // Title
    let mut y = text_start_y;
    for tline in &title_lines {
        if y < inner.y + inner.height {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    tline.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ))),
                Rect::new(inner.x, y, inner.width, 1),
            );
            y += 1;
        }
    }

    // Channel
    if y < inner.y + inner.height {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                truncate_str(&channel, w),
                Style::default().fg(Color::DarkGray),
            ))),
            Rect::new(inner.x, y, inner.width, 1),
        );
        y += 1;
    }

    // Meta
    if y < inner.y + inner.height {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                truncate_str(&meta, w),
                Style::default().fg(Color::DarkGray),
            ))),
            Rect::new(inner.x, y, inner.width, 1),
        );
    }
}

fn render_thumbnail(f: &mut Frame, item: &FeedItem, area: Rect, thumb_cache: &ThumbnailCache) {
    let key = item.thumbnail_key();
    if let Some(img) = thumb_cache.get(&key) {
        // Render actual thumbnail using half-block characters
        ThumbnailCache::render_halfblock(img, area, f.buffer_mut());
    } else {
        // Fallback: colored placeholder
        render_thumb_placeholder(f, item, area);
    }
}

fn render_thumb_placeholder(f: &mut Frame, item: &FeedItem, area: Rect) {
    // Generate a color based on the item's ID for visual variety
    let id = match item {
        FeedItem::Video(v) | FeedItem::Short(v) => &v.id,
        FeedItem::Channel(c) => &c.id,
        FeedItem::Playlist(p) => &p.id,
    };
    let hash = simple_hash(id);

    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Yellow,
        Color::LightRed,
        Color::LightBlue,
    ];
    let bg_color = colors[hash % colors.len()];

    // Fill the area with half-block characters using the color
    for y in 0..area.height {
        let row = "\u{2580}".repeat(area.width as usize);
        let span = Span::styled(row, Style::default().fg(bg_color).bg(Color::Black));
        f.render_widget(
            Paragraph::new(Line::from(span)),
            Rect::new(area.x, area.y + y, area.width, 1),
        );
    }
}

/// Returns (title, channel, views/meta, time_ago)
fn format_card_item(item: &FeedItem) -> (String, String, String, String) {
    match item {
        FeedItem::Video(v) | FeedItem::Short(v) => {
            let views = v
                .view_count
                .map(|n| format!("{} views", format_count(n)))
                .unwrap_or_default();
            let time_ago = v.published.map(format_time_ago).unwrap_or_default();
            (v.title.clone(), v.channel.clone(), views, time_ago)
        }
        FeedItem::Channel(c) => {
            let subs = c.subscriber_count.map(format_count).unwrap_or_default();
            (
                c.name.clone(),
                "Channel".into(),
                format!("{} subs", subs),
                String::new(),
            )
        }
        FeedItem::Playlist(p) => {
            let count = p
                .video_count
                .map(|n| format!("{} videos", n))
                .unwrap_or_default();
            (p.title.clone(), p.channel.clone(), count, String::new())
        }
    }
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

fn simple_hash(s: &str) -> usize {
    s.bytes().fold(0usize, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as usize)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_count() {
        assert_eq!(format_count(500), "500");
        assert_eq!(format_count(1_500), "1.5K");
        assert_eq!(format_count(2_300_000), "2.3M");
        assert_eq!(format_count(1_200_000_000), "1.2B");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello...");
        assert_eq!(truncate_str("ab", 2), "ab");
        assert_eq!(truncate_str("abcd", 3), "abc");
        // Unicode safety
        assert_eq!(truncate_str("こんにちは世界", 6), "こんに...");
    }

    #[test]
    fn test_simple_hash_deterministic() {
        let h1 = simple_hash("abc");
        let h2 = simple_hash("abc");
        assert_eq!(h1, h2);
        assert_ne!(simple_hash("abc"), simple_hash("xyz"));
    }
}
