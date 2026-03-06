use crate::app::AppState;
use crate::player::PlayerState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    if let Some(load) = &state.playback_loading {
        let spinner = loading_spinner(load.started_at.elapsed().as_millis());
        let elapsed = load.started_at.elapsed().as_secs_f32();
        let status = if load.slow {
            "Still loading"
        } else {
            "Loading"
        };
        let text = Paragraph::new(format!(
            "{} {} \"{}\"  Q: {}  {:.1}s",
            spinner,
            status,
            load.label,
            state.playback_quality.label(),
            elapsed,
        ))
        .style(Style::default().fg(if load.slow {
            Color::Yellow
        } else {
            Color::Cyan
        }))
        .block(Block::default().borders(Borders::ALL).title("Now Playing"));
        f.render_widget(text, area);
        return;
    }

    match &state.player_state {
        PlayerState::Stopped => {
            let text = Paragraph::new(format!(
                "No media playing  Quality: {}",
                state.playback_quality.label()
            ))
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Now Playing"));
            f.render_widget(text, area);
        }
        PlayerState::Playing(info) | PlayerState::Paused(info) => {
            let is_paused = matches!(&state.player_state, PlayerState::Paused(_));
            let icon = if is_paused { "❚❚" } else { "▶" };

            let time_current = format_time(info.time_pos);
            let time_total = format_time(info.duration);
            let ratio = if info.duration > 0.0 {
                (info.time_pos / info.duration).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Split the area into: info line + progress bar
            let inner = Block::default().borders(Borders::ALL).title("Now Playing");
            let inner_area = inner.inner(area);
            f.render_widget(inner, area);

            if inner_area.height < 1 {
                return;
            }

            let label = format!(
                "{} {} — {}  {}/{}  Vol: {}%  Q: {}",
                icon,
                info.title,
                if is_paused { "[PAUSED]" } else { "" },
                time_current,
                time_total,
                info.volume as u32,
                state.playback_quality.label(),
            );

            let gauge = Gauge::default()
                .label(label)
                .ratio(ratio)
                .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray));

            f.render_widget(gauge, inner_area);
        }
    }
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
