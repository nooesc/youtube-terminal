use crate::app::{Action, AppState, Direction, PopupState, Tab};
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
    // If popup is active, handle popup input
    if state.popup.is_some() {
        return map_popup_key(key, state);
    }

    // If command mode is active, handle command input
    if state.command.active {
        return map_command_key(key);
    }

    // If search bar is focused, handle text input
    if state.search.focused {
        return map_search_key(key);
    }

    // If filter mode is active, handle filter navigation
    if state.search.filter.active {
        return map_filter_key(key);
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),

        // Search
        KeyCode::Char('/') | KeyCode::Char('s') => Some(Action::FocusSearch),

        // Tab switching
        KeyCode::Char('1') => Some(Action::SwitchTab(Tab::ForYou)),
        KeyCode::Char('2') => Some(Action::SwitchTab(Tab::SavedSearches)),
        KeyCode::Char('3') => Some(Action::SwitchTab(Tab::Subscriptions)),
        KeyCode::Char('4') => Some(Action::SwitchTab(Tab::History)),
        KeyCode::Tab => {
            let next = match state.tabs.active {
                Tab::ForYou => Tab::SavedSearches,
                Tab::SavedSearches => Tab::Subscriptions,
                Tab::Subscriptions => Tab::History,
                Tab::History => Tab::ForYou,
            };
            Some(Action::SwitchTab(next))
        }
        KeyCode::BackTab => {
            let prev = match state.tabs.active {
                Tab::ForYou => Tab::History,
                Tab::SavedSearches => Tab::ForYou,
                Tab::Subscriptions => Tab::SavedSearches,
                Tab::History => Tab::Subscriptions,
            };
            Some(Action::SwitchTab(prev))
        }

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
        KeyCode::Char('Q') => Some(Action::TogglePlaybackQuality),
        KeyCode::Char('X') => Some(Action::StopPlayerAndQuit),

        // Subscribe/Unsubscribe toggle (or save search in search view)
        KeyCode::Char('S') => {
            if state.view == crate::app::View::Search {
                Some(Action::OpenSaveSearchPopup)
            } else {
                Some(Action::SubscribeSelected)
            }
        }

        // Filter mode (only in search view)
        KeyCode::Char('f') if state.view == crate::app::View::Search => {
            Some(Action::EnterFilterMode)
        }

        // Saved search actions (delete/rename in SavedSearches tab)
        KeyCode::Char('d') if state.view == crate::app::View::Home && state.tabs.active == Tab::SavedSearches => {
            Some(Action::OpenDeleteSearchConfirm)
        }
        KeyCode::Char('r') if state.view == crate::app::View::Home && state.tabs.active == Tab::SavedSearches => {
            Some(Action::OpenRenameSearchPopup)
        }

        // Command mode
        KeyCode::Char(':') => Some(Action::EnterCommandMode),

        _ => None,
    }
}

fn map_popup_key(key: KeyEvent, state: &AppState) -> Option<Action> {
    match &state.popup {
        Some(PopupState::ConfirmDelete { .. }) => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => Some(Action::PopupSubmit),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(Action::PopupCancel),
            _ => None,
        },
        Some(PopupState::SaveSearch { .. } | PopupState::Rename { .. }) => match key.code {
            KeyCode::Enter => Some(Action::PopupSubmit),
            KeyCode::Esc => Some(Action::PopupCancel),
            KeyCode::Backspace => Some(Action::PopupBackspace),
            KeyCode::Char(c) => Some(Action::PopupInput(c)),
            _ => None,
        },
        None => None,
    }
}

fn map_filter_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('r') => Some(Action::ResetFilters),
        KeyCode::Esc | KeyCode::Char('f') => Some(Action::ExitFilterMode),
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => Some(Action::FilterNextField),
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => Some(Action::FilterPrevField),
        KeyCode::Up | KeyCode::Char('k') => Some(Action::FilterCycleUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::FilterCycleDown),
        KeyCode::Enter => Some(Action::ExitFilterMode),
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

fn map_command_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::CancelCommand),
        KeyCode::Enter => None, // SubmitCommand is handled specially in poll_event
        KeyCode::Backspace => Some(Action::CommandBackspace),
        KeyCode::Char(c) => Some(Action::CommandInput(c)),
        _ => None,
    }
}

/// Poll for the next action from crossterm events or the async channel.
/// Returns None if no event occurred within the timeout.
pub fn poll_event(state: &AppState) -> Option<Action> {
    if event::poll(Duration::from_millis(50)).ok()? {
        if let Event::Key(key) = event::read().ok()? {
            // Special case: Enter in command mode submits the command
            if state.command.active && key.code == KeyCode::Enter {
                if !state.command.input.is_empty() {
                    return Some(Action::SubmitCommand(state.command.input.clone()));
                }
                return Some(Action::CancelCommand);
            }
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
