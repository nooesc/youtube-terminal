use crate::app::{AppState, Tab};
use ratatui::prelude::*;
use ratatui::widgets::Tabs;

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let titles = vec!["For You", "Subscriptions", "History"];
    let selected = match state.tabs.active {
        Tab::ForYou => 0,
        Tab::Subscriptions => 1,
        Tab::History => 2,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .divider("|");

    f.render_widget(tabs, area);
}
