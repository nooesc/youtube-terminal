use crate::models::*;
use crate::player::PlayerState as MpvPlayerState;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Home,
    Search,
    VideoDetail(String),
    ChannelDetail(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    ForYou,
    Subscriptions,
    History,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug)]
pub enum Action {
    // Navigation
    SwitchTab(Tab),
    Navigate(Direction),
    Select,
    Back,

    // Search
    FocusSearch,
    UnfocusSearch,
    SubmitSearch(String),
    SearchInput(char),
    SearchBackspace,

    // Playback
    PlayVideo(String),
    PlayAudio(String),
    TogglePause,
    Seek(f64),
    VolumeUp,
    VolumeDown,

    // Async results
    FeedLoaded(u64, Box<LoadedPage>),
    SearchResults(u64, FeedPage<FeedItem>),
    DetailLoaded(u64, VideoDetail),
    ThumbnailReady(ThumbnailKey, PathBuf),
    PlayerStateUpdate(MpvPlayerState),

    // App
    Quit,
}

#[derive(Debug)]
pub enum LoadedPage {
    Home(FeedPage<FeedItem>),
    Subscriptions(FeedPage<ChannelItem>),
    SubscriptionFeed(FeedPage<VideoItem>),
    History(FeedPage<HistoryEntry>),
    Trending(FeedPage<VideoItem>),
}

pub struct TabState {
    pub active: Tab,
}

pub struct SearchState {
    pub query: String,
    pub cursor: usize,
    pub focused: bool,
}

pub struct CardGridState {
    pub items: Vec<FeedItem>,
    pub selected_row: usize,
    pub selected_col: usize,
    pub columns: usize,
    pub continuation: Option<String>,
}

pub struct VideoListState {
    pub items: Vec<FeedItem>,
    pub selected: usize,
    pub continuation: Option<String>,
}

pub struct DetailState {
    pub detail: VideoDetail,
    pub selected_action: usize,
}

pub struct LoadingState {
    pub feed_loading: bool,
    pub feed_request_id: u64,
    pub search_loading: bool,
    pub search_request_id: u64,
    pub detail_loading: bool,
    pub detail_request_id: u64,
    pub thumbnail_loading: HashSet<ThumbnailKey>,
}

pub struct AppState {
    pub view: View,
    pub previous_views: Vec<View>,
    pub tabs: TabState,
    pub search: SearchState,
    pub cards: CardGridState,
    pub video_list: VideoListState,
    pub detail: Option<DetailState>,
    pub player_state: MpvPlayerState,
    pub loading: LoadingState,
    pub should_quit: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            view: View::Home,
            previous_views: Vec::new(),
            tabs: TabState { active: Tab::ForYou },
            search: SearchState {
                query: String::new(),
                cursor: 0,
                focused: false,
            },
            cards: CardGridState {
                items: Vec::new(),
                selected_row: 0,
                selected_col: 0,
                columns: 3,
                continuation: None,
            },
            video_list: VideoListState {
                items: Vec::new(),
                selected: 0,
                continuation: None,
            },
            detail: None,
            player_state: MpvPlayerState::Stopped,
            loading: LoadingState {
                feed_loading: false,
                feed_request_id: 0,
                search_loading: false,
                search_request_id: 0,
                detail_loading: false,
                detail_request_id: 0,
                thumbnail_loading: HashSet::new(),
            },
            should_quit: false,
        }
    }

    /// Process an action and update state accordingly.
    /// Returns nothing; async side-effects are handled by the caller.
    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::SwitchTab(tab) => {
                self.tabs.active = tab;
                self.view = View::Home;
                // Caller should trigger feed load for the new tab
            }
            Action::Navigate(dir) => match self.view {
                View::Home => self.navigate_cards(dir),
                View::Search => self.navigate_list(dir),
                View::VideoDetail(_) => self.navigate_detail(dir),
                View::ChannelDetail(_) => {}
            },
            Action::Select => {
                self.handle_select();
            }
            Action::Back => {
                if let Some(prev) = self.previous_views.pop() {
                    self.view = prev;
                }
            }
            Action::FocusSearch => {
                self.search.focused = true;
                self.previous_views.push(self.view.clone());
                self.view = View::Search;
            }
            Action::UnfocusSearch => {
                self.search.focused = false;
                if let Some(prev) = self.previous_views.pop() {
                    self.view = prev;
                }
            }
            Action::SubmitSearch(_query) => {
                self.search.focused = false;
                self.loading.search_request_id += 1;
                self.loading.search_loading = true;
                self.video_list.items.clear();
                self.video_list.selected = 0;
                // Caller spawns the actual search task
            }
            Action::SearchInput(ch) => {
                self.search.query.insert(self.search.cursor, ch);
                self.search.cursor += ch.len_utf8();
            }
            Action::SearchBackspace => {
                if self.search.cursor > 0 {
                    let prev = self.search.query[..self.search.cursor]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    let start = self.search.cursor - prev;
                    self.search.query.drain(start..self.search.cursor);
                    self.search.cursor = start;
                }
            }
            // Playback actions are handled by the event loop, not dispatch
            Action::PlayVideo(_)
            | Action::PlayAudio(_)
            | Action::TogglePause
            | Action::Seek(_)
            | Action::VolumeUp
            | Action::VolumeDown => {}
            Action::FeedLoaded(req_id, page) => {
                if req_id == self.loading.feed_request_id {
                    self.loading.feed_loading = false;
                    match *page {
                        LoadedPage::Home(feed) => {
                            self.cards.items = feed.items;
                            self.cards.continuation = feed.continuation;
                        }
                        LoadedPage::Trending(feed) => {
                            self.cards.items =
                                feed.items.into_iter().map(FeedItem::Video).collect();
                            self.cards.continuation = feed.continuation;
                        }
                        LoadedPage::SubscriptionFeed(feed) => {
                            self.cards.items =
                                feed.items.into_iter().map(FeedItem::Video).collect();
                            self.cards.continuation = feed.continuation;
                        }
                        LoadedPage::History(feed) => {
                            self.cards.items = feed
                                .items
                                .into_iter()
                                .map(|e| FeedItem::Video(e.video))
                                .collect();
                            self.cards.continuation = feed.continuation;
                        }
                        LoadedPage::Subscriptions(_) => {
                            // Channel list display handled separately
                        }
                    }
                    self.cards.selected_row = 0;
                    self.cards.selected_col = 0;
                }
            }
            Action::SearchResults(req_id, page) => {
                if req_id == self.loading.search_request_id {
                    self.loading.search_loading = false;
                    self.video_list.items = page.items;
                    self.video_list.continuation = page.continuation;
                    self.video_list.selected = 0;
                }
            }
            Action::DetailLoaded(req_id, detail) => {
                if req_id == self.loading.detail_request_id {
                    self.loading.detail_loading = false;
                    let video_id = detail.item.id.clone();
                    self.detail = Some(DetailState {
                        detail,
                        selected_action: 0,
                    });
                    self.previous_views.push(self.view.clone());
                    self.view = View::VideoDetail(video_id);
                }
            }
            Action::ThumbnailReady(key, _path) => {
                self.loading.thumbnail_loading.remove(&key);
            }
            Action::PlayerStateUpdate(state) => {
                self.player_state = state;
            }
            Action::Quit => {
                self.should_quit = true;
            }
        }
    }

    fn navigate_cards(&mut self, dir: Direction) {
        let total = self.cards.items.len();
        if total == 0 {
            return;
        }
        let cols = self.cards.columns.max(1);
        let rows = (total + cols - 1) / cols;

        match dir {
            Direction::Left => {
                if self.cards.selected_col > 0 {
                    self.cards.selected_col -= 1;
                }
            }
            Direction::Right => {
                let max_col =
                    (cols - 1).min(total.saturating_sub(1) - self.cards.selected_row * cols);
                if self.cards.selected_col < max_col {
                    self.cards.selected_col += 1;
                }
            }
            Direction::Up => {
                if self.cards.selected_row > 0 {
                    self.cards.selected_row -= 1;
                }
            }
            Direction::Down => {
                if self.cards.selected_row < rows.saturating_sub(1) {
                    self.cards.selected_row += 1;
                    // Clamp column if new row has fewer items
                    let items_in_row = if self.cards.selected_row == rows - 1 {
                        total - self.cards.selected_row * cols
                    } else {
                        cols
                    };
                    self.cards.selected_col =
                        self.cards.selected_col.min(items_in_row.saturating_sub(1));
                }
            }
        }
    }

    fn navigate_list(&mut self, dir: Direction) {
        let total = self.video_list.items.len();
        if total == 0 {
            return;
        }
        match dir {
            Direction::Up => {
                self.video_list.selected = self.video_list.selected.saturating_sub(1);
            }
            Direction::Down => {
                if self.video_list.selected < total - 1 {
                    self.video_list.selected += 1;
                }
            }
            _ => {}
        }
    }

    fn navigate_detail(&mut self, dir: Direction) {
        if let Some(ref mut detail) = self.detail {
            let max_actions = 3; // Play Video, Play Audio, Open Channel
            match dir {
                Direction::Up => {
                    detail.selected_action = detail.selected_action.saturating_sub(1);
                }
                Direction::Down => {
                    if detail.selected_action < max_actions - 1 {
                        detail.selected_action += 1;
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_select(&mut self) {
        match self.view {
            View::Search => {
                // Select a video from search results -> trigger detail load
                // The actual loading is handled by the event loop
            }
            View::Home => {
                // Select a card -> trigger detail load
            }
            View::VideoDetail(_) => {
                // Execute selected action (play video, play audio, etc.)
                // Handled by event loop
            }
            View::ChannelDetail(_) => {}
        }
    }

    /// Get the currently selected item index in the card grid.
    pub fn selected_card_index(&self) -> usize {
        self.cards.selected_row * self.cards.columns + self.cards.selected_col
    }

    /// Get the currently selected FeedItem from the card grid, if any.
    pub fn selected_card_item(&self) -> Option<&FeedItem> {
        self.cards.items.get(self.selected_card_index())
    }

    /// Get the currently selected FeedItem from the video list, if any.
    pub fn selected_list_item(&self) -> Option<&FeedItem> {
        self.video_list.items.get(self.video_list.selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let state = AppState::new();
        assert_eq!(state.view, View::Home);
        assert_eq!(state.tabs.active, Tab::ForYou);
        assert!(!state.should_quit);
    }

    #[test]
    fn test_switch_tab() {
        let mut state = AppState::new();
        state.dispatch(Action::SwitchTab(Tab::History));
        assert_eq!(state.tabs.active, Tab::History);
    }

    #[test]
    fn test_quit() {
        let mut state = AppState::new();
        state.dispatch(Action::Quit);
        assert!(state.should_quit);
    }

    #[test]
    fn test_focus_search() {
        let mut state = AppState::new();
        state.dispatch(Action::FocusSearch);
        assert_eq!(state.view, View::Search);
        assert!(state.search.focused);
    }

    #[test]
    fn test_back_pops_view() {
        let mut state = AppState::new();
        state.dispatch(Action::FocusSearch);
        assert_eq!(state.view, View::Search);
        state.dispatch(Action::Back);
        assert_eq!(state.view, View::Home);
    }

    #[test]
    fn test_stale_request_ignored() {
        let mut state = AppState::new();
        state.loading.search_request_id = 5;
        // Old request with id=3 should be ignored
        state.dispatch(Action::SearchResults(
            3,
            FeedPage {
                items: vec![],
                continuation: None,
            },
        ));
        assert_eq!(state.video_list.items.len(), 0);
    }

    #[test]
    fn test_search_input() {
        let mut state = AppState::new();
        state.dispatch(Action::SearchInput('h'));
        state.dispatch(Action::SearchInput('i'));
        assert_eq!(state.search.query, "hi");
        assert_eq!(state.search.cursor, 2);
    }

    #[test]
    fn test_search_backspace() {
        let mut state = AppState::new();
        state.dispatch(Action::SearchInput('a'));
        state.dispatch(Action::SearchInput('b'));
        state.dispatch(Action::SearchBackspace);
        assert_eq!(state.search.query, "a");
    }

    #[test]
    fn test_navigate_list() {
        let mut state = AppState::new();
        state.video_list.items = vec![
            FeedItem::Video(VideoItem {
                id: "1".into(),
                title: "A".into(),
                channel: "".into(),
                channel_id: "".into(),
                view_count: None,
                duration: None,
                published: None,
                thumbnail_url: "".into(),
            }),
            FeedItem::Video(VideoItem {
                id: "2".into(),
                title: "B".into(),
                channel: "".into(),
                channel_id: "".into(),
                view_count: None,
                duration: None,
                published: None,
                thumbnail_url: "".into(),
            }),
        ];
        state.view = View::Search;
        state.dispatch(Action::Navigate(Direction::Down));
        assert_eq!(state.video_list.selected, 1);
        state.dispatch(Action::Navigate(Direction::Down));
        assert_eq!(state.video_list.selected, 1); // can't go past end
        state.dispatch(Action::Navigate(Direction::Up));
        assert_eq!(state.video_list.selected, 0);
    }
}
