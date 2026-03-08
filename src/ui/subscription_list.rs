use crate::app::AppState;
use crate::models::{ItemType, ThumbnailKey};
use crate::thumbnails::ThumbnailCache;
use crate::ui::theme;
use ratatui::prelude::*;
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

pub const AVATAR_SIZE: u16 = 24;
const AVATAR_PANEL_WIDTH: u16 = 30;

pub fn render(f: &mut Frame, state: &AppState, area: Rect, thumb_cache: &ThumbnailCache) {
    if state.loading.feed_loading {
        let loading = Paragraph::new("  Loading subscriptions...")
            .style(Style::default().fg(theme::WARNING));
        f.render_widget(loading, area);
        return;
    }

    if state.subscription_channels.is_empty() {
        let empty = Paragraph::new(
            "  No subscriptions yet. Search for channels and press S to subscribe.",
        )
        .style(Style::default().fg(theme::TEXT_DIM));
        f.render_widget(empty, area);
        return;
    }

    // Split: list on left, avatar panel on right (if wide enough)
    let show_avatar = area.width > 60;
    let (list_area, avatar_area) = if show_avatar {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(30),
                Constraint::Length(AVATAR_PANEL_WIDTH),
            ])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    // Render channel list
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
                Span::styled(marker, Style::default().fg(theme::ACCENT)),
                Span::styled(
                    &channel.name,
                    Style::default()
                        .fg(theme::TEXT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(meta, Style::default().fg(theme::TEXT_DIM)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.cards.selected_row));

    let list = List::new(items).highlight_style(Style::default().bg(theme::SELECTED_BG));

    f.render_stateful_widget(list, list_area, &mut list_state);

    // Render avatar panel for selected channel
    if let Some(avatar_area) = avatar_area {
        if let Some(channel) = state.subscription_channels.get(state.cards.selected_row) {
            render_avatar_panel(f, channel, avatar_area, thumb_cache);
        }
    }
}

fn render_avatar_panel(
    f: &mut Frame,
    channel: &crate::models::ChannelItem,
    area: Rect,
    thumb_cache: &ThumbnailCache,
) {
    let key = ThumbnailKey {
        item_type: ItemType::Channel,
        item_id: channel.id.clone(),
    };

    // Avatar image area (square, centered)
    let avatar_w = area.width.min(AVATAR_SIZE);
    let avatar_h = (avatar_w / 2).min(area.height.saturating_sub(3)); // leave room for text
    let avatar_x = area.x + (area.width.saturating_sub(avatar_w)) / 2;

    if avatar_h > 0 {
        let avatar_rect = Rect::new(avatar_x, area.y, avatar_w, avatar_h);

        if let Some(img) = thumb_cache.get_avatar(&key) {
            ThumbnailCache::render_halfblock(img, avatar_rect, f.buffer_mut());
        } else {
            // Placeholder
            render_avatar_placeholder(f, channel, avatar_rect);
        }
    }

    // Channel name below avatar
    let text_y = area.y + avatar_h + 1;
    if text_y < area.y + area.height {
        let name = truncate_str(&channel.name, area.width as usize);
        let name_x = area.x + (area.width.saturating_sub(name.chars().count() as u16)) / 2;
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                name,
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ))),
            Rect::new(name_x, text_y, area.width.saturating_sub(name_x - area.x), 1),
        );
    }

    // Subscriber count below name
    let sub_y = area.y + avatar_h + 2;
    if sub_y < area.y + area.height {
        if let Some(count) = channel.subscriber_count {
            let subs = format!("{} subscribers", format_count(count));
            let sub_x = area.x + (area.width.saturating_sub(subs.len() as u16)) / 2;
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    subs,
                    Style::default().fg(theme::TEXT_DIM),
                ))),
                Rect::new(sub_x, sub_y, area.width.saturating_sub(sub_x - area.x), 1),
            );
        }
    }
}

fn render_avatar_placeholder(f: &mut Frame, channel: &crate::models::ChannelItem, area: Rect) {
    // Show first letter of channel name in a colored box
    let initial = channel
        .name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    let hash = simple_hash(&channel.id);
    let colors = [
        Color::Rgb(95, 175, 95),   // green
        Color::Rgb(135, 175, 223), // blue
        Color::Rgb(215, 135, 95),  // orange
        Color::Rgb(175, 95, 175),  // purple
        Color::Rgb(95, 175, 175),  // teal
        Color::Rgb(215, 175, 95),  // gold
    ];
    let bg_color = colors[hash % colors.len()];

    // Fill area with the color
    for y in 0..area.height {
        for x in 0..area.width {
            if let Some(cell) =
                f.buffer_mut().cell_mut(Position::new(area.x + x, area.y + y))
            {
                cell.set_char(' ').set_bg(bg_color);
            }
        }
    }

    // Draw initial letter in center
    let center_x = area.x + area.width / 2;
    let center_y = area.y + area.height / 2;
    if let Some(cell) = f.buffer_mut().cell_mut(Position::new(center_x, center_y)) {
        cell.set_char(initial.chars().next().unwrap_or('?'))
            .set_fg(Color::White)
            .set_bg(bg_color);
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
        format!("{truncated}...")
    } else {
        s.chars().take(max).collect()
    }
}

fn simple_hash(s: &str) -> usize {
    s.bytes()
        .fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize))
}
