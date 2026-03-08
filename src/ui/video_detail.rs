use crate::app::AppState;
use crate::models::{ItemType, ThumbnailKey};
use crate::thumbnails::ThumbnailCache;
use crate::ui::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

/// Size to load detail thumbnails at (columns x rows).
pub const DETAIL_THUMB_W: u32 = 80;
pub const DETAIL_THUMB_H: u32 = 30;

pub fn render(f: &mut Frame, state: &AppState, area: Rect, thumb_cache: &ThumbnailCache) {
    let detail_state = match &state.detail {
        Some(d) => d,
        None => {
            if state.loading.detail_loading {
                let loading = Paragraph::new("  Loading video details...")
                    .style(Style::default().fg(theme::WARNING));
                f.render_widget(loading, area);
            }
            return;
        }
    };

    let detail = &detail_state.detail;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // header
            Constraint::Min(5),   // middle: thumbnail + description
            Constraint::Length(7), // actions
        ])
        .split(area);

    render_header(f, detail, chunks[0]);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // thumbnail
            Constraint::Percentage(60), // description
        ])
        .split(chunks[1]);

    render_detail_thumbnail(f, detail, middle[0], thumb_cache);
    render_description(f, &detail.description, middle[1]);
    render_actions(f, detail_state.selected_action, chunks[2]);
}

fn render_header(f: &mut Frame, detail: &crate::models::VideoDetail, area: Rect) {
    let views = detail.item.view_count.map(format_count).unwrap_or_default();
    let likes = detail.like_count.map(format_count).unwrap_or_default();

    let stats = if !likes.is_empty() {
        format!("{} views \u{00b7} {} likes", views, likes)
    } else {
        format!("{} views", views)
    };

    let text = vec![
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
            &detail.item.channel,
            Style::default().fg(theme::CHANNEL),
        )),
        Line::from(Span::styled(stats, Style::default().fg(theme::TEXT_DIM))),
    ];

    let header = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(theme::BORDER)),
    );
    f.render_widget(header, area);
}

fn render_detail_thumbnail(
    f: &mut Frame,
    detail: &crate::models::VideoDetail,
    area: Rect,
    thumb_cache: &ThumbnailCache,
) {
    if area.height < 2 || area.width < 4 {
        return;
    }

    let key = ThumbnailKey {
        item_type: ItemType::Video,
        item_id: detail.item.id.clone(),
    };

    if let Some(img) = thumb_cache.get_detail(&key) {
        ThumbnailCache::render_halfblock(img, area, f.buffer_mut());
    } else if let Some(img) = thumb_cache.get(&key) {
        ThumbnailCache::render_halfblock(img, area, f.buffer_mut());
    } else {
        let placeholder = Paragraph::new("No thumbnail")
            .style(Style::default().fg(theme::TEXT_DIM))
            .alignment(Alignment::Center);
        let y_center = area.y + area.height / 2;
        f.render_widget(
            placeholder,
            Rect::new(area.x, y_center, area.width, 1),
        );
    }
}

fn render_description(f: &mut Frame, description: &str, area: Rect) {
    let desc = Paragraph::new(description)
        .style(Style::default().fg(theme::TEXT))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::LEFT)
                .border_style(Style::default().fg(theme::BORDER)),
        );
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
    list_state.select(Some(selected));

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme::BORDER)),
    );

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
