use crate::app::{AppState, Tab};
use crate::ui::theme;
use ratatui::prelude::*;
use ratatui::widgets::Tabs;

pub fn render(f: &mut Frame, state: &AppState, area: Rect) {
    let titles = vec!["For You", "Saved Searches", "Subscriptions", "History"];
    let selected = match state.tabs.active {
        Tab::ForYou => 0,
        Tab::SavedSearches => 1,
        Tab::Subscriptions => 2,
        Tab::History => 3,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(Style::default().fg(theme::TEXT_DIM))
        .highlight_style(
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider("\u{2502}");

    f.render_widget(tabs, area);
}
