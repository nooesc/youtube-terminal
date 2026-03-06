use crate::app::AppState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn render(f: &mut Frame, state: &AppState, area: Rect, channel_id: &str) {
    let detail_state = match &state.channel_detail {
        Some(detail) if detail.detail.item.id == channel_id => detail,
        _ => {
            let loading = Paragraph::new("Loading channel details...")
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::ALL).title("Channel"));
            f.render_widget(loading, area);
            return;
        }
    };

    let detail = &detail_state.detail;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(5)])
        .split(area);

    let subs = detail
        .item
        .subscriber_count
        .map(format_count)
        .unwrap_or_else(|| "hidden".to_string());
    let videos = detail
        .video_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            &detail.item.name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{subs} subscribers · {videos} videos"),
            Style::default().fg(Color::Cyan),
        )),
    ])
    .block(Block::default().borders(Borders::ALL).title("Channel"));
    f.render_widget(header, chunks[0]);

    let body = Paragraph::new(detail.description.as_str())
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("About"));
    f.render_widget(body, chunks[1]);
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
