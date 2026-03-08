use crate::app::{AppState, SearchDate, SearchItemType, SearchLength, SearchSort};
use crate::db::saved_searches::SavedSearch;
use crate::ui::theme;
use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    if state.saved_searches.items.is_empty() {
        let empty =
            Paragraph::new("  No saved searches yet. Press / to search, then S to save.")
                .style(Style::default().fg(theme::TEXT_DIM));
        f.render_widget(empty, area);
        return;
    }

    let total = state.saved_searches.items.len();
    let lines_per_item: u16 = 2;
    let visible = (area.height / lines_per_item) as usize;

    if visible == 0 {
        return;
    }

    let scroll_offset = if state.saved_searches.selected >= visible {
        state.saved_searches.selected - visible + 1
    } else {
        0
    };

    for i in 0..visible {
        let idx = i + scroll_offset;
        if idx >= total {
            break;
        }

        let item = &state.saved_searches.items[idx];
        let is_selected = idx == state.saved_searches.selected;
        let y = area.y + (i as u16) * lines_per_item;
        let w = area.width as usize;

        render_item(f, item, is_selected, area.x, y, area.width, w);
    }
}

fn render_item(
    f: &mut Frame,
    item: &SavedSearch,
    selected: bool,
    x: u16,
    y: u16,
    width: u16,
    w: usize,
) {
    let bg = if selected {
        theme::SELECTED_BG
    } else {
        Color::Reset
    };

    // Line 1: marker + name + "query text"
    let marker = if selected { "\u{25b8} " } else { "  " };
    let query_display = format!("\"{}\"", item.query);

    // Reserve space: 2 (marker) + name + 2 (gap) + query
    let max_name = w.saturating_sub(2 + 2 + query_display.chars().count());
    let name_truncated = truncate_str(&item.name, max_name);
    let name_display_len = name_truncated.chars().count();

    let mut line1_spans: Vec<Span> = vec![
        Span::styled(marker, Style::default().fg(theme::ACCENT).bg(bg)),
        Span::styled(
            name_truncated,
            Style::default()
                .fg(theme::TEXT)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default().bg(bg)),
        Span::styled(
            truncate_str(&query_display, w.saturating_sub(2 + name_display_len + 2)),
            Style::default().fg(theme::TEXT_DIM).bg(bg),
        ),
    ];

    // Pad line 1 to full width
    let line1_content_len =
        2 + name_display_len + 2 + query_display.chars().count().min(w.saturating_sub(2 + name_display_len + 2));
    let line1_pad = w.saturating_sub(line1_content_len);
    if line1_pad > 0 {
        line1_spans.push(Span::styled(
            " ".repeat(line1_pad),
            Style::default().bg(bg),
        ));
    }

    f.render_widget(
        Paragraph::new(Line::from(line1_spans)),
        Rect::new(x, y, width, 1),
    );

    // Line 2: indented filter summary + last run
    let filter_summary = build_filter_summary(item);
    let last_run = format_last_run(&item.last_run_at);

    let mut line2_spans: Vec<Span> = vec![Span::styled("    ", Style::default().bg(bg))];

    if !filter_summary.is_empty() {
        line2_spans.push(Span::styled(
            filter_summary.clone(),
            Style::default().fg(theme::WARNING).bg(bg),
        ));
        line2_spans.push(Span::styled(
            "  \u{00b7}  ",
            Style::default().fg(theme::TEXT_DIM).bg(bg),
        ));
    }

    line2_spans.push(Span::styled(
        last_run.clone(),
        Style::default().fg(theme::TEXT_DIM).bg(bg),
    ));

    // Pad line 2 to full width
    let line2_content_len = 4
        + if filter_summary.is_empty() {
            0
        } else {
            filter_summary.chars().count() + 5
        }
        + last_run.chars().count();
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

fn build_filter_summary(item: &SavedSearch) -> String {
    let mut parts: Vec<String> = Vec::new();

    if item.sort != SearchSort::Relevance {
        parts.push(format!("Sort: {}", item.sort.label()));
    }
    if item.date != SearchDate::Any {
        parts.push(format!("Date: {}", item.date.label()));
    }
    if item.item_type != SearchItemType::All {
        parts.push(format!("Type: {}", item.item_type.label()));
    }
    if item.length != SearchLength::Any {
        parts.push(format!("Length: {}", item.length.label()));
    }

    parts.join(" \u{00b7} ")
}

fn format_last_run(last_run_at: &Option<String>) -> String {
    match last_run_at {
        None => "never run".to_string(),
        Some(ts) => {
            let parsed = chrono::DateTime::parse_from_rfc3339(ts)
                .or_else(|_| chrono::DateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ"));
            match parsed {
                Ok(dt) => {
                    let ago = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
                    if ago.num_minutes() < 1 {
                        "ran just now".to_string()
                    } else if ago.num_hours() < 1 {
                        format!("ran {}m ago", ago.num_minutes())
                    } else if ago.num_hours() < 24 {
                        format!("ran {}h ago", ago.num_hours())
                    } else if ago.num_days() < 30 {
                        format!("ran {}d ago", ago.num_days())
                    } else if ago.num_days() < 365 {
                        format!("ran {}mo ago", ago.num_days() / 30)
                    } else {
                        format!("ran {}y ago", ago.num_days() / 365)
                    }
                }
                Err(_) => "never run".to_string(),
            }
        }
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
