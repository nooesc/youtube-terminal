use crate::app::AppState;
use crate::models::FeedItem;
use crate::thumbnails::ThumbnailCache;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

const CARD_WIDTH: u16 = 26;
const CARD_HEIGHT: u16 = 10;
const THUMB_HEIGHT: u16 = 4;

pub fn render(f: &mut Frame, state: &AppState, area: Rect, thumb_cache: &ThumbnailCache) {
    if state.loading.feed_loading && state.cards.items.is_empty() {
        let loading = Paragraph::new("Loading...")
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Content"));
        f.render_widget(loading, area);
        return;
    }

    if state.cards.items.is_empty() {
        let empty = Paragraph::new("No content yet. Press / to search.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Content"));
        f.render_widget(empty, area);
        return;
    }

    let outer = Block::default().borders(Borders::ALL).title("Content");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

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
    let border_style = if selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 4 {
        return;
    }

    // Thumbnail placeholder (colored based on item ID hash)
    let (title, channel, meta) = format_card_item(item);
    let thumb_h = THUMB_HEIGHT.min(inner.height.saturating_sub(3));
    let thumb_area = Rect::new(inner.x, inner.y, inner.width, thumb_h);
    render_thumbnail(f, item, thumb_area, thumb_cache);

    // Title (below thumbnail)
    let text_y = inner.y + thumb_area.height;
    let text_width = inner.width;

    if text_y < inner.y + inner.height {
        let truncated_title = truncate_str(&title, text_width as usize);
        let title_span = Span::styled(
            truncated_title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(
            Paragraph::new(Line::from(title_span)),
            Rect::new(inner.x, text_y, text_width, 1),
        );
    }

    // Channel
    if text_y + 1 < inner.y + inner.height {
        let truncated_channel = truncate_str(&channel, text_width as usize);
        let ch_span = Span::styled(truncated_channel, Style::default().fg(Color::DarkGray));
        f.render_widget(
            Paragraph::new(Line::from(ch_span)),
            Rect::new(inner.x, text_y + 1, text_width, 1),
        );
    }

    // Meta (views + time)
    if text_y + 2 < inner.y + inner.height {
        let truncated_meta = truncate_str(&meta, text_width as usize);
        let meta_span = Span::styled(truncated_meta, Style::default().fg(Color::DarkGray));
        f.render_widget(
            Paragraph::new(Line::from(meta_span)),
            Rect::new(inner.x, text_y + 2, text_width, 1),
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

fn format_card_item(item: &FeedItem) -> (String, String, String) {
    match item {
        FeedItem::Video(v) | FeedItem::Short(v) => {
            let views = v
                .view_count
                .map(|n| format!("{} views", format_count(n)))
                .unwrap_or_default();
            (v.title.clone(), v.channel.clone(), views)
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
    if s.len() <= max {
        s.to_string()
    } else if max > 3 {
        format!("{}...", &s[..max - 3])
    } else {
        s[..max].to_string()
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
    }

    #[test]
    fn test_simple_hash_deterministic() {
        let h1 = simple_hash("abc");
        let h2 = simple_hash("abc");
        assert_eq!(h1, h2);
        assert_ne!(simple_hash("abc"), simple_hash("xyz"));
    }
}
