use crate::models::*;
use crate::player::{PlaybackQuality, PlaybackSession, PlayerState as MpvPlayerState};
use crate::session::PendingSessionRestore;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum View {
    Home,
    Search,
    VideoDetail(String),
    ChannelDetail(String),
    PlaylistDetail(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[allow(dead_code)]
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
    TogglePlaybackQuality,
    PlaybackLoadSlow(u64),
    StopPlayer,
    StopPlayerAndQuit,

    // Async results
    FeedLoaded(u64, Box<LoadedPage>),
    SearchResults(u64, FeedPage<FeedItem>),
    AppendFeed(u64, Box<LoadedPage>),
    AppendSearch(u64, FeedPage<FeedItem>),
    DetailLoaded(u64, VideoDetail),
    ChannelDetailLoaded(u64, ChannelDetail),
    PlaylistDetailLoaded(u64, PlaylistDetail),
    ThumbnailReady(ThumbnailKey, PathBuf),
    ThumbnailFailed(ThumbnailKey),
    PlayerStateUpdate(MpvPlayerState),

    // Command mode
    EnterCommandMode,
    CommandInput(char),
    CommandBackspace,
    SubmitCommand(String),
    CancelCommand,

    // Subscriptions
    Subscribe(ChannelItem),
    Unsubscribe(String), // channel_id
    SubscribeSelected,

    // Errors
    ShowError(String),

    // App
    Quit,
}

#[derive(Debug)]
pub enum LoadedPage {
    Home(FeedPage<FeedItem>),
    Subscriptions(FeedPage<ChannelItem>),
    History(FeedPage<HistoryEntry>),
}

pub struct TabState {
    pub active: Tab,
}

pub struct SearchState {
    pub query: String,
    pub cursor: usize,
    pub focused: bool,
}

pub struct CommandState {
    pub active: bool,
    pub input: String,
    pub message: Option<String>,
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

#[allow(dead_code)]
pub struct ChannelDetailState {
    pub detail: ChannelDetail,
    pub selected_action: usize,
    pub selected_video: usize,
    pub is_subscribed: bool,
}

pub struct PlaylistDetailState {
    pub detail: PlaylistDetail,
    pub selected_action: usize,
}

pub struct LoadingState {
    pub feed_loading: bool,
    pub feed_request_id: u64,
    pub search_loading: bool,
    pub search_request_id: u64,
    pub detail_loading: bool,
    pub detail_request_id: u64,
    pub loading_more_feed: bool,
    pub loading_more_search: bool,
    pub thumbnail_loading: HashSet<ThumbnailKey>,
    pub playback_request_id: u64,
}

pub struct PlaybackLoadState {
    pub request_id: u64,
    pub label: String,
    pub started_at: Instant,
    pub slow: bool,
}

pub struct AppState {
    pub view: View,
    pub previous_views: Vec<View>,
    pub tabs: TabState,
    pub search: SearchState,
    pub command: CommandState,
    pub cards: CardGridState,
    pub video_list: VideoListState,
    pub detail: Option<DetailState>,
    pub channel_detail: Option<ChannelDetailState>,
    pub playlist_detail: Option<PlaylistDetailState>,
    pub subscription_channels: Vec<ChannelItem>,
    pub player_state: MpvPlayerState,
    pub playback_quality: PlaybackQuality,
    pub current_playback: Option<PlaybackSession>,
    pub playback_loading: Option<PlaybackLoadState>,
    pub pending_restore: Option<PendingSessionRestore>,
    pub loading: LoadingState,
    pub stop_player_on_exit: bool,
    pub should_quit: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            view: View::Home,
            previous_views: Vec::new(),
            tabs: TabState {
                active: Tab::ForYou,
            },
            search: SearchState {
                query: String::new(),
                cursor: 0,
                focused: false,
            },
            command: CommandState {
                active: false,
                input: String::new(),
                message: None,
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
            channel_detail: None,
            playlist_detail: None,
            subscription_channels: Vec::new(),
            player_state: MpvPlayerState::Stopped,
            playback_quality: PlaybackQuality::P1080,
            current_playback: None,
            playback_loading: None,
            pending_restore: None,
            loading: LoadingState {
                feed_loading: false,
                feed_request_id: 0,
                search_loading: false,
                search_request_id: 0,
                detail_loading: false,
                detail_request_id: 0,
                loading_more_feed: false,
                loading_more_search: false,
                thumbnail_loading: HashSet::new(),
                playback_request_id: 0,
            },
            stop_player_on_exit: false,
            should_quit: false,
        }
    }

    /// Update the number of grid columns based on current terminal width.
    pub fn update_columns(&mut self, terminal_width: u16) {
        let card_width = crate::ui::card_grid::CARD_WIDTH + 1; // +1 gap
        self.cards.columns = ((terminal_width.saturating_sub(2)) / card_width).max(1) as usize;
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
                View::Home => {
                    if self.tabs.active == Tab::Subscriptions {
                        self.navigate_subscription_list(dir);
                    } else {
                        self.navigate_cards(dir);
                    }
                }
                View::Search => self.navigate_list(dir),
                View::VideoDetail(_) => self.navigate_detail(dir),
                View::PlaylistDetail(_) => self.navigate_playlist_detail(dir),
                View::ChannelDetail(_) => self.navigate_channel_detail(dir),
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
            Action::EnterCommandMode => {
                self.command.active = true;
                self.command.input.clear();
                self.command.message = None;
            }
            Action::CommandInput(ch) => {
                self.command.input.push(ch);
            }
            Action::CommandBackspace => {
                self.command.input.pop();
            }
            Action::SubmitCommand(_) => {
                self.command.active = false;
                // Actual command execution is handled by the caller
            }
            Action::CancelCommand => {
                self.command.active = false;
                self.command.input.clear();
                self.command.message = None;
            }
            // Playback actions are handled by the event loop, not dispatch
            Action::PlayVideo(_)
            | Action::PlayAudio(_)
            | Action::TogglePause
            | Action::Seek(_)
            | Action::VolumeUp
            | Action::VolumeDown
            | Action::TogglePlaybackQuality
            | Action::StopPlayer
            | Action::StopPlayerAndQuit => {}
            Action::PlaybackLoadSlow(req_id) => {
                if let Some(ref mut load) = self.playback_loading {
                    if load.request_id == req_id {
                        load.slow = true;
                    }
                }
            }
            Action::FeedLoaded(req_id, page) => {
                if req_id == self.loading.feed_request_id {
                    self.loading.feed_loading = false;
                    match *page {
                        LoadedPage::Home(feed) => {
                            self.cards.items = feed.items;
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
                        LoadedPage::Subscriptions(feed) => {
                            self.subscription_channels = feed.items;
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
            Action::AppendFeed(req_id, page) => {
                if req_id == self.loading.feed_request_id {
                    self.loading.loading_more_feed = false;
                    match *page {
                        LoadedPage::Home(feed) => {
                            self.cards.items.extend(feed.items);
                            self.cards.continuation = feed.continuation;
                        }
                        LoadedPage::History(feed) => {
                            self.cards
                                .items
                                .extend(feed.items.into_iter().map(|e| FeedItem::Video(e.video)));
                            self.cards.continuation = feed.continuation;
                        }
                        LoadedPage::Subscriptions(_) => {}
                    }
                }
            }
            Action::AppendSearch(req_id, page) => {
                if req_id == self.loading.search_request_id {
                    self.loading.loading_more_search = false;
                    self.video_list.items.extend(page.items);
                    self.video_list.continuation = page.continuation;
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
                    if !matches!(&self.view, View::VideoDetail(current) if current == &video_id) {
                        self.previous_views.push(self.view.clone());
                        self.view = View::VideoDetail(video_id);
                    }
                }
            }
            Action::ChannelDetailLoaded(req_id, detail) => {
                if req_id == self.loading.detail_request_id {
                    self.loading.detail_loading = false;
                    let channel_id = detail.item.id.clone();
                    self.channel_detail = Some(ChannelDetailState {
                        detail,
                        selected_action: 0,
                        selected_video: 0,
                        is_subscribed: false, // Will be set by caller after dispatch
                    });
                    if !matches!(&self.view, View::ChannelDetail(current) if current == &channel_id)
                    {
                        self.previous_views.push(self.view.clone());
                        self.view = View::ChannelDetail(channel_id);
                    }
                }
            }
            Action::PlaylistDetailLoaded(req_id, detail) => {
                if req_id == self.loading.detail_request_id {
                    self.loading.detail_loading = false;
                    let playlist_id = detail.item.id.clone();
                    self.playlist_detail = Some(PlaylistDetailState {
                        detail,
                        selected_action: 0,
                    });
                    if !matches!(
                        &self.view,
                        View::PlaylistDetail(current) if current == &playlist_id
                    ) {
                        self.previous_views.push(self.view.clone());
                        self.view = View::PlaylistDetail(playlist_id);
                    }
                }
            }
            Action::ThumbnailReady(key, _path) => {
                self.loading.thumbnail_loading.remove(&key);
            }
            Action::ThumbnailFailed(key) => {
                self.loading.thumbnail_loading.remove(&key);
            }
            Action::PlayerStateUpdate(state) => {
                if !matches!(state, MpvPlayerState::Stopped) {
                    self.playback_loading = None;
                } else if self.playback_loading.is_none() {
                    self.current_playback = None;
                }
                self.player_state = state;
            }
            Action::Subscribe(_) | Action::Unsubscribe(_) | Action::SubscribeSelected => {
                // Handled by the event loop in main.rs, not by dispatch
            }
            Action::ShowError(msg) => {
                self.command.message = Some(msg);
                // Clear loading flags so the UI doesn't stay in a loading state
                self.loading.feed_loading = false;
                self.loading.search_loading = false;
                self.loading.detail_loading = false;
                self.loading.loading_more_feed = false;
                self.loading.loading_more_search = false;
                self.playback_loading = None;
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
        let rows = total.div_ceil(cols);

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

    fn navigate_subscription_list(&mut self, dir: Direction) {
        let total = self.subscription_channels.len();
        if total == 0 {
            return;
        }
        match dir {
            Direction::Up => {
                self.cards.selected_row = self.cards.selected_row.saturating_sub(1);
            }
            Direction::Down => {
                if self.cards.selected_row < total - 1 {
                    self.cards.selected_row += 1;
                }
            }
            _ => {}
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

    fn navigate_playlist_detail(&mut self, dir: Direction) {
        if let Some(ref mut detail) = self.playlist_detail {
            let max_actions = 3; // Play Playlist, Play Audio, Open Channel
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

    fn navigate_channel_detail(&mut self, dir: Direction) {
        if let Some(ref mut cd) = self.channel_detail {
            let num_videos = cd.detail.videos.len();
            match dir {
                Direction::Up => {
                    if cd.selected_action == 1 && cd.selected_video > 0 {
                        cd.selected_video -= 1;
                    } else if cd.selected_action == 1 && cd.selected_video == 0 {
                        cd.selected_action = 0; // go back to subscribe button
                    }
                }
                Direction::Down => {
                    if cd.selected_action == 0 {
                        if num_videos > 0 {
                            cd.selected_action = 1; // enter videos section
                            cd.selected_video = 0;
                        }
                    } else if cd.selected_action == 1
                        && cd.selected_video < num_videos.saturating_sub(1)
                    {
                        cd.selected_video += 1;
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
            View::PlaylistDetail(_) => {}
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
        assert_eq!(state.playback_quality, PlaybackQuality::P1080);
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

    fn make_video(id: &str) -> FeedItem {
        FeedItem::Video(VideoItem {
            id: id.into(),
            title: id.into(),
            channel: "".into(),
            channel_id: "".into(),
            view_count: None,
            duration: None,
            published: None,
            thumbnail_url: "".into(),
        })
    }

    #[test]
    fn test_append_search_extends_items() {
        let mut state = AppState::new();
        state.loading.search_request_id = 1;
        // Simulate initial search results
        state.dispatch(Action::SearchResults(
            1,
            FeedPage {
                items: vec![make_video("1"), make_video("2")],
                continuation: Some("token_a".into()),
            },
        ));
        assert_eq!(state.video_list.items.len(), 2);
        assert_eq!(state.video_list.continuation, Some("token_a".into()));

        // Navigate to item 1
        state.view = View::Search;
        state.dispatch(Action::Navigate(Direction::Down));
        assert_eq!(state.video_list.selected, 1);

        // Simulate continuation append
        state.loading.loading_more_search = true;
        state.dispatch(Action::AppendSearch(
            1,
            FeedPage {
                items: vec![make_video("3"), make_video("4")],
                continuation: Some("token_b".into()),
            },
        ));

        // Items should be appended, not replaced
        assert_eq!(state.video_list.items.len(), 4);
        assert_eq!(state.video_list.continuation, Some("token_b".into()));
        // Selected index should NOT be reset
        assert_eq!(state.video_list.selected, 1);
        assert!(!state.loading.loading_more_search);
    }

    #[test]
    fn test_append_search_stale_request_ignored() {
        let mut state = AppState::new();
        state.loading.search_request_id = 5;
        state.video_list.items = vec![make_video("1")];

        // Stale request should be ignored
        state.dispatch(Action::AppendSearch(
            3,
            FeedPage {
                items: vec![make_video("x")],
                continuation: None,
            },
        ));
        assert_eq!(state.video_list.items.len(), 1);
    }

    #[test]
    fn test_append_feed_extends_items() {
        let mut state = AppState::new();
        state.loading.feed_request_id = 1;
        // Simulate initial feed
        state.dispatch(Action::FeedLoaded(
            1,
            Box::new(LoadedPage::Home(FeedPage {
                items: vec![make_video("1")],
                continuation: Some("feed_token".into()),
            })),
        ));
        assert_eq!(state.cards.items.len(), 1);

        // Simulate continuation append
        state.loading.loading_more_feed = true;
        state.dispatch(Action::AppendFeed(
            1,
            Box::new(LoadedPage::Home(FeedPage {
                items: vec![make_video("2"), make_video("3")],
                continuation: None,
            })),
        ));

        assert_eq!(state.cards.items.len(), 3);
        assert_eq!(state.cards.continuation, None);
        assert!(!state.loading.loading_more_feed);
    }
}
