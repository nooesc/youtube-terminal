use crate::app::{Action, AppState, Direction, Tab};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tokio::sync::mpsc;

pub type ActionSender = mpsc::UnboundedSender<Action>;
pub type ActionReceiver = mpsc::UnboundedReceiver<Action>;

pub fn create_action_channel() -> (ActionSender, ActionReceiver) {
    mpsc::unbounded_channel()
}

/// Map a key event to an Action based on current view state
pub fn map_key_event(key: KeyEvent, state: &AppState) -> Option<Action> {
    // If search bar is focused, handle text input
    if state.search.focused {
        return map_search_key(key);
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),

        // Search
        KeyCode::Char('/') | KeyCode::Char('s') => Some(Action::FocusSearch),

        // Tab switching
        KeyCode::Char('1') => Some(Action::SwitchTab(Tab::ForYou)),
        KeyCode::Char('2') => Some(Action::SwitchTab(Tab::Subscriptions)),
        KeyCode::Char('3') => Some(Action::SwitchTab(Tab::History)),

        // Navigation (vim keys)
        KeyCode::Char('h') | KeyCode::Left => Some(Action::Navigate(Direction::Left)),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::Navigate(Direction::Down)),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::Navigate(Direction::Up)),
        KeyCode::Char('l') | KeyCode::Right => Some(Action::Navigate(Direction::Right)),

        // Select / Enter
        KeyCode::Enter => Some(Action::Select),

        // Back
        KeyCode::Esc => Some(Action::Back),

        // Playback controls
        KeyCode::Char(' ') => Some(Action::TogglePause),
        KeyCode::Char('>') => Some(Action::Seek(10.0)),
        KeyCode::Char('<') => Some(Action::Seek(-10.0)),
        KeyCode::Char('+') | KeyCode::Char('=') => Some(Action::VolumeUp),
        KeyCode::Char('-') => Some(Action::VolumeDown),

        _ => None,
    }
}

fn map_search_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::UnfocusSearch),
        KeyCode::Enter => None, // SubmitSearch is handled specially — needs the query string
        KeyCode::Backspace => Some(Action::SearchBackspace),
        KeyCode::Char(c) => Some(Action::SearchInput(c)),
        _ => None,
    }
}

/// Poll for the next action from crossterm events or the async channel.
/// Returns None if no event occurred within the timeout.
pub fn poll_event(state: &AppState) -> Option<Action> {
    if event::poll(Duration::from_millis(50)).ok()? {
        if let Event::Key(key) = event::read().ok()? {
            // Special case: Enter in search mode submits the query
            if state.search.focused && key.code == KeyCode::Enter {
                if !state.search.query.is_empty() {
                    return Some(Action::SubmitSearch(state.search.query.clone()));
                }
                return None;
            }
            return map_key_event(key, state);
        }
    }
    None
}
