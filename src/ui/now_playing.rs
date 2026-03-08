use crate::app::AppState;
use crate::player::PlayerState;
use crate::ui::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme::BORDER))
        .title(Line::from(Span::styled(
            " Now Playing ",
            Style::default().fg(theme::TEXT_DIM),
        )));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 1 {
        return;
    }

    if let Some(load) = &state.playback_loading {
        render_loading(f, state, load, inner);
        return;
    }

    match &state.player_state {
        PlayerState::Stopped => {
            let text = format!(
                "  Nothing playing  Q: {}",
                state.playback_quality.label()
            );
            let para = Paragraph::new(text).style(Style::default().fg(theme::TEXT_DIM));
            f.render_widget(para, Rect::new(inner.x, inner.y, inner.width, 1));
        }
        PlayerState::Playing(info) | PlayerState::Paused(info) => {
            let is_paused = matches!(&state.player_state, PlayerState::Paused(_));
            let icon = if is_paused { "\u{275a}\u{275a}" } else { "\u{25b6}" };
            let paused_tag = if is_paused { " [PAUSED]" } else { "" };

            let time_current = format_time(info.time_pos);
            let time_total = format_time(info.duration);
            let ratio = if info.duration > 0.0 {
                (info.time_pos / info.duration).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let info_line = format!(
                "  {} {}{}  Vol: {}%  Q: {}",
                icon,
                info.title,
                paused_tag,
                info.volume as u32,
                state.playback_quality.label(),
            );
            let info_style = if is_paused {
                Style::default().fg(theme::WARNING)
            } else {
                Style::default().fg(theme::TEXT)
            };
            f.render_widget(
                Paragraph::new(info_line).style(info_style),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );

            if inner.height >= 2 {
                let time_label = format!(" {}/{}", time_current, time_total);
                let bar_width =
                    inner.width.saturating_sub(time_label.len() as u16 + 2) as usize;
                let filled = (ratio * bar_width as f64) as usize;
                let empty = bar_width.saturating_sub(filled);

                let bar_spans = vec![
                    Span::raw("  "),
                    Span::styled(
                        "\u{2501}".repeat(filled),
                        Style::default().fg(theme::ACCENT),
                    ),
                    Span::styled(
                        "\u{2500}".repeat(empty),
                        Style::default().fg(theme::BORDER),
                    ),
                    Span::styled(time_label, Style::default().fg(theme::TEXT_DIM)),
                ];
                f.render_widget(
                    Paragraph::new(Line::from(bar_spans)),
                    Rect::new(inner.x, inner.y + 1, inner.width, 1),
                );
            }
        }
    }
}

fn render_loading(
    f: &mut Frame,
    state: &AppState,
    load: &crate::app::PlaybackLoadState,
    area: Rect,
) {
    let spinner = loading_spinner(load.started_at.elapsed().as_millis());
    let elapsed = load.started_at.elapsed().as_secs_f32();
    let status = if load.slow {
        "Still loading"
    } else {
        "Loading"
    };
    let text = format!(
        "  {} {} \"{}\"  Q: {}  {:.1}s",
        spinner,
        status,
        load.label,
        state.playback_quality.label(),
        elapsed,
    );
    let style = if load.slow {
        Style::default().fg(theme::WARNING)
    } else {
        Style::default().fg(theme::CHANNEL)
    };
    f.render_widget(
        Paragraph::new(text).style(style),
        Rect::new(area.x, area.y, area.width, 1),
    );
}

fn loading_spinner(elapsed_ms: u128) -> &'static str {
    const FRAMES: [&str; 8] = [
        "[    ]", "[=   ]", "[==  ]", "[=== ]", "[ ===]", "[  ==]", "[   =]", "[    ]",
    ];
    let idx = ((elapsed_ms / 120) as usize) % FRAMES.len();
    FRAMES[idx]
}

fn format_time(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}
