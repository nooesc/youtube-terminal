# youtube-terminal — Design Document

**Date:** 2026-03-05
**Status:** Draft (v2 — revised after Codex review)
**License:** GPL-3.0-or-later (required by RustyPipe dependency)

## Overview

A terminal-based YouTube client built in Rust. Browse your subscription feed, search videos, and track local watch history — all from the terminal. Video playback via mpv in a separate window (PiP style), audio-only mode for background listening. Designed for tmux + Alacritty users.

## Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Performance, safety, great TUI ecosystem |
| TUI framework | ratatui + crossterm | De facto standard for Rust TUIs |
| Async runtime | tokio | Required by RustyPipe, mature ecosystem |
| YouTube data | RustyPipe (InnerTube API) | Native Rust, no API key, supports auth via cookies |
| Playback | mpv via JSON IPC | Battle-tested, yt-dlp integration built-in, IPC for full control |
| Persistence | SQLite (via rusqlite) | Single file, embedded, good for history/metadata index |
| Thumbnail cache | Filesystem (`~/.cache/youtube-terminal/thumbs/`) | Avoids SQLite bloat; SQLite indexes paths only |
| Thumbnails | Half-block ASCII art | Only reliable option for Alacritty + tmux |
| Config | TOML | Standard for Rust projects |

## Authentication

### Primary: Cookie File Import (v1)

1. First launch prompts user for a Netscape-format cookie file path (e.g. exported from Firefox via a browser extension).
2. App copies cookie file to `~/.local/share/youtube-terminal/session/cookies.txt` with `chmod 600`.
3. Cookies fed into RustyPipe via `user_auth_set_cookie_txt`.
4. Same cookie file path passed to mpv/yt-dlp via `--ytdl-raw-options=cookies=<path>` for playback auth consistency.
5. Validated on startup by making a lightweight authenticated RustyPipe call; prompt re-import if expired.

**v1 scope: cookie.txt import only.** Browser-native extraction (reading browser SQLite DBs, handling Chrome encryption, profile discovery) is explicitly deferred. Headless browser bootstrapping is a future enhancement.

### Fallback: No Auth

When no valid cookies are available, the app still works:
- "For You" tab falls back to Trending (via `RustyPipeQuery::trending()`)
- History tab shows local-only history (what you've played through the app)
- Subscriptions tab shows "login required" message with instructions
- Search works fully without auth

### Cookie File Lifecycle

- Stored at: `~/.local/share/youtube-terminal/session/cookies.txt`
- Permissions: `0600` (owner read/write only)
- mpv/yt-dlp read this file directly — no keychain indirection
- RustyPipe cache stored separately at `~/.local/share/youtube-terminal/rustypipe/`
- Re-import command available via `:import-cookies <path>` command

### Account / Profile Selection

RustyPipe's cookie auth resolves `X-Goog-AuthUser` and `X-Goog-PageId` headers for multi-profile and brand account cases. For v1:

- Assume the cookies map to a single YouTube identity (the default profile)
- If the user has multiple YouTube profiles (brand accounts), the app uses whichever profile the cookies were exported from
- Future: add `:switch-profile` command + profile picker if multi-account is needed
- Document in setup instructions that users should export cookies while their desired profile is active

## UI Layout

### Homepage (Default View)

```
┌─────────────────────────────────────────────────────────┐
│ / Search...                                             │
├────────────┬──────────────────┬─────────────────────────┤
│  For You   │  Subscriptions   │  History                │
├────────────┴──────────────────┴─────────────────────────┤
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ ▀▄█▀▄█▀▄ │  │ ▀▄█▀▄█▀▄ │  │ ▀▄█▀▄█▀▄ │              │
│  │ ▀▄THUMB▄ │  │ ▀▄THUMB▄ │  │ ▀▄THUMB▄ │              │
│  ├──────────┤  ├──────────┤  ├──────────┤              │
│  │ Title    │  │ Title    │  │ Title    │              │
│  │ Channel  │  │ Channel  │  │ Channel  │              │
│  │ 1.2M 3mo │  │ 500K 1d  │  │ 80K 2hr  │              │
│  └──────────┘  └──────────┘  └──────────┘              │
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ ▀▄█▀▄█▀▄ │  │ ▀▄█▀▄█▀▄ │  │ ▀▄█▀▄█▀▄ │              │
│  │ ...      │  │ ...      │  │ ...      │              │
│  └──────────┘  └──────────┘  └──────────┘              │
│                                                         │
├─────────────────────────────────────────────────────────┤
│ ▶ Now Playing: Title — Channel      ├──────┤ 2:30/4:15 │
└─────────────────────────────────────────────────────────┘
```

- Card grid is responsive — number of columns adapts to terminal width
- Cards contain: half-block thumbnail, title (truncated), channel name, view count, relative upload date
- Scroll vertically through rows of cards
- Pagination: load next page when scrolling near the bottom (continuation token based)

### Search Mode

Activated by focusing the search bar (`/` or `s`). Replaces the tab content area (not the now-playing bar).

```
┌─────────────────────────────────────────────────────────┐
│ / rust programming tutorials                      [ESC] │
├─────────────────────────────────────────────────────────┤
│ ▸ Rust in 100 Seconds — Fireship          1.2M · 2y    │
│   Rust for Beginners — Let's Get Rusty    800K · 1y    │
│   Rust Crash Course — Traversy Media      500K · 3y    │
│   Why Rust? — ThePrimeagen               300K · 6mo    │
│   ...                                                   │
├─────────────────────────────────────────────────────────┤
│ ▶ Now Playing: Title — Channel      ├──────┤ 2:30/4:15 │
└─────────────────────────────────────────────────────────┘
```

- Results displayed as a compact vertical list (no thumbnails — speed and density matter here)
- j/k to navigate, Enter to select, ESC to return to previous tab
- Load more results on reaching the bottom (continuation token)

### Video Detail View

When a video is selected from any list or card grid:

```
┌─────────────────────────────────────────────────────────┐
│ ← Back (ESC)                                            │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  Title of the Video                                     │
│  Channel Name · 1.2M views · 3 months ago               │
│                                                         │
│  Description text here, possibly truncated or           │
│  scrollable...                                          │
│                                                         │
│  ─────────────────────────────────────────               │
│  Actions:                                               │
│  ▸ Play Video (mpv window)                              │
│    Play Audio Only                                      │
│    Open Channel                                         │
│    Download (yt-dlp)                                    │
│                                                         │
├─────────────────────────────────────────────────────────┤
│ ▶ Now Playing: Title — Channel      ├──────┤ 2:30/4:15 │
└─────────────────────────────────────────────────────────┘
```

Note: "Add to Queue" is deferred from v1. Queue management requires a fully designed PlayerState with playlist semantics, which is out of scope for the initial build.

## Playback Architecture

### Player Lifecycle

The app manages a **single mpv process at a time**. When the user plays something new, the existing mpv process is stopped and replaced.

- Each app instance uses a unique IPC socket: `/tmp/yt-term-{pid}.sock`
- On startup, clean up any stale sockets from previous crashed instances
- On quit, send `quit` command via IPC and clean up the socket file

### Video Playback (External mpv Window)

```bash
mpv \
  --geometry=400x225+0+0 \
  --ontop \
  --input-ipc-server=/tmp/yt-term-$$.sock \
  --ytdl-raw-options=cookies=$HOME/.local/share/youtube-terminal/session/cookies.txt \
  'https://www.youtube.com/watch?v=<id>'
```

- Separate OS window, picture-in-picture style
- User can resize/reposition the mpv window
- TUI remains fully interactive

### Audio-Only Playback

```bash
mpv \
  --no-video \
  --input-ipc-server=/tmp/yt-term-$$.sock \
  --ytdl-raw-options=cookies=$HOME/.local/share/youtube-terminal/session/cookies.txt \
  'https://www.youtube.com/watch?v=<id>'
```

- No window opened
- Audio streams in background
- Controlled entirely via IPC from TUI

### mpv JSON IPC Control

Communication over Unix socket at `/tmp/yt-term-{pid}.sock`:

```json
{"command": ["loadfile", "https://youtube.com/watch?v=..."]}
{"command": ["set_property", "pause", true]}
{"command": ["get_property", "time-pos"]}
{"command": ["get_property", "duration"]}
{"command": ["get_property", "media-title"]}
{"command": ["seek", "10"]}
{"command": ["set_property", "volume", 80]}
```

Now-playing bar polls `time-pos`, `duration`, and `media-title` on a 1-second tick via a background tokio task.

## Data Layer

### Feed Model

YouTube feeds are paginated and can contain mixed item types. The data layer models this explicitly rather than flattening to `Vec<VideoItem>`.

```rust
/// A page of results from YouTube with an optional continuation token
pub struct FeedPage<T> {
    pub items: Vec<T>,
    pub continuation: Option<String>,
}

/// Items that can appear in feeds — not just videos
pub enum FeedItem {
    Video(VideoItem),
    Channel(ChannelItem),
    Playlist(PlaylistItem),
    Short(VideoItem),
}

pub struct VideoItem {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub channel_id: String,
    pub view_count: Option<u64>,
    pub duration: Option<Duration>,
    pub published: Option<DateTime<Utc>>,
    pub thumbnail_url: String,
}

pub struct VideoDetail {
    pub item: VideoItem,
    pub description: String,
    pub like_count: Option<u64>,
    pub keywords: Vec<String>,
}

pub struct ChannelItem {
    pub id: String,
    pub name: String,
    pub subscriber_count: Option<u64>,
    pub thumbnail_url: String,
}

pub struct PlaylistItem {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub video_count: Option<u32>,
    pub thumbnail_url: String,
}

/// History items carry extra metadata about when the video was watched
pub struct HistoryEntry {
    pub video: VideoItem,
    pub watched_at: DateTime<Utc>,
}
```

### ContentProvider Trait

```rust
#[async_trait]
pub trait ContentProvider {
    fn capabilities(&self) -> AuthCapabilities;

    // Unauthenticated
    async fn search(&self, query: &str, filters: &SearchFilters, continuation: Option<&str>) -> Result<FeedPage<FeedItem>>;
    async fn trending(&self) -> Result<FeedPage<VideoItem>>;
    async fn video_detail(&self, id: &str) -> Result<VideoDetail>;
    async fn channel(&self, id: &str) -> Result<ChannelDetail>;
    async fn channel_videos(&self, id: &str, continuation: Option<&str>) -> Result<FeedPage<VideoItem>>;

    // Authenticated (require cookies)
    async fn home_feed(&self, continuation: Option<&str>) -> Result<FeedPage<FeedItem>>;
    async fn subscriptions(&self, continuation: Option<&str>) -> Result<FeedPage<ChannelItem>>;
    async fn subscription_feed(&self, continuation: Option<&str>) -> Result<FeedPage<VideoItem>>;
    async fn watch_history(&self, continuation: Option<&str>) -> Result<FeedPage<HistoryEntry>>;
}

pub struct AuthCapabilities {
    pub has_home_feed: bool,
    pub has_subscriptions: bool,
    pub has_history: bool,
    pub has_private_playback: bool,
}
```

### "For You" / Home Feed

RustyPipe 0.11.4 does **not** have a first-class home feed API in its userdata module. The available authenticated endpoints are: `history`, `subscriptions`, `subscription_feed`, `saved_playlists`, `liked_videos`, `watch_later`.

For the "For You" tab, the implementation strategy is:

1. **With auth:** Attempt a raw InnerTube `browse` request with `browseId: "FEwhat_to_watch"` using RustyPipe's authenticated client. This is the internal YouTube homepage endpoint. If RustyPipe doesn't expose this directly, we'll need to make a custom HTTP request with the auth cookies and parse the InnerTube response ourselves.
2. **Without auth:** Fall back to `RustyPipeQuery::trending()` which works unauthenticated.
3. **Risk:** The raw InnerTube approach may be fragile. If it proves too brittle, the "For You" tab becomes "Trending" permanently, which is an acceptable degradation.

## State Management

### AppState

Single owned struct — no global mutables, no `static mut`, no `OnceLock` hacks.

```rust
pub struct AppState {
    pub view: View,
    pub previous_views: Vec<View>,
    pub tabs: TabState,
    pub search: SearchState,
    pub cards: CardGridState,
    pub video_list: VideoListState,
    pub detail: Option<DetailState>,
    pub player: PlayerState,
    pub auth: AuthState,
    pub loading: LoadingState,  // tracks in-flight async operations
}

pub enum View {
    Home,
    Search,
    VideoDetail(String),
    ChannelDetail(String),
}

/// Loading state for async operations
pub struct LoadingState {
    pub feed_loading: bool,
    pub feed_request_id: u64,
    pub search_loading: bool,
    pub search_request_id: u64,
    pub detail_loading: bool,
    pub detail_request_id: u64,
    pub thumbnail_loading: HashSet<ThumbnailKey>,  // typed item keys with in-flight downloads
}

pub enum Action {
    // Navigation
    SwitchTab(Tab),
    Navigate(Direction),
    Select,
    Back,

    // Search
    FocusSearch,
    SubmitSearch(String),

    // Playback
    PlayVideo(String),
    PlayAudio(String),
    TogglePause,
    Seek(f64),
    VolumeUp,
    VolumeDown,

    // Async results (u64 is a request_id to detect stale responses)
    FeedLoaded(u64, Result<LoadedPage>),
    SearchResults(u64, Result<FeedPage<FeedItem>>),
    DetailLoaded(u64, Result<VideoDetail>),
    ThumbnailReady(ThumbnailKey, PathBuf),
    PlayerStateUpdate(PlayerInfo),
    AuthValidated(bool),
}

/// Typed page results — each tab returns different item types
pub enum LoadedPage {
    Home(FeedPage<FeedItem>),
    Subscriptions(FeedPage<ChannelItem>),
    SubscriptionFeed(FeedPage<VideoItem>),
    History(FeedPage<HistoryEntry>),
    Trending(FeedPage<VideoItem>),
}

/// Typed cache key for thumbnails — not just video IDs
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ThumbnailKey {
    pub item_type: ItemType,
    pub item_id: String,
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum ItemType {
    Video,
    Channel,
    Playlist,
}
```

### Async Architecture

Network calls, image downloads, and DB access run on background tokio tasks. The UI thread never blocks on I/O.

```
┌─────────────────────────────────────────────┐
│                 UI Thread                    │
│                                             │
│  loop {                                     │
│    1. Poll crossterm events (100ms timeout) │
│    2. Drain mpsc channel for async results  │
│    3. Map events + results → Actions        │
│    4. Dispatch Actions → mutate AppState    │
│    5. Render UI from AppState               │
│  }                                          │
└────────────────┬────────────────────────────┘
                 │ mpsc::channel
┌────────────────┴────────────────────────────┐
│            Background Tasks (tokio)          │
│                                             │
│  - Feed/search/detail fetches via RustyPipe │
│  - Thumbnail downloads (bounded concurrency)│
│  - DB writes (rusqlite via spawn_blocking)  │
│  - mpv IPC polling (1s tick)                │
│  - Auth validation                          │
└─────────────────────────────────────────────┘
```

- `rusqlite` is synchronous — all DB access wrapped in `tokio::task::spawn_blocking`
- Thumbnail downloads use `tokio::sync::Semaphore` for bounded concurrency (max 4 concurrent)
- Stale results are dropped via `request_id`: each new request increments `LoadingState.{feed,search,detail}_request_id`, and async result Actions carry the originating ID. The dispatcher ignores results whose ID doesn't match the current state
- AppState includes `LoadingState` to show spinners/placeholders in the UI

### Event Loop Detail

```
loop {
    // 1. Poll terminal events with short timeout for responsive UI
    if let Ok(true) = crossterm::event::poll(Duration::from_millis(100)) {
        let event = crossterm::event::read()?;
        // map to Action
    }

    // 2. Drain async results from background tasks
    while let Ok(action) = result_rx.try_recv() {
        dispatch(action, &mut state);
    }

    // 3. Render
    terminal.draw(|f| ui::render(f, &state))?;
}
```

## Persistence (SQLite)

Single database at `~/.local/share/youtube-terminal/data.db`.

### Tables

- `watch_history` — video_id, title, channel, channel_id, watched_at, duration_watched, thumbnail_url
- `metadata_cache` — video_id, json_data, fetched_at (TTL: 24 hours)
- `settings` — key/value store for preferences

### Thumbnail Cache (Filesystem)

- Location: `~/.cache/youtube-terminal/thumbs/{item_type}_{item_id}.jpg` (e.g. `video_abc123.jpg`, `channel_xyz.jpg`, `playlist_def.jpg`)
- SQLite `thumbnail_index` table: item_id, item_type, file_path, fetched_at
- Bounded by age (30 days) and count (1000 files)
- Cleanup on startup: delete files older than 30 days, prune index
- Filesystem storage avoids SQLite bloat, WAL pressure, and slow vacuum

### History

**v1: local-only.** The `watch_history` table tracks videos played through the app. This is not synced with YouTube's account history.

RustyPipe's `watch_history()` endpoint can read the user's remote YouTube history (with auth), but:
- Remote history is read-only (we can't mark videos as watched on YouTube)
- Merging local + remote history adds complexity with no clear UX benefit for v1

The History tab in v1 shows **local playback history only**. Remote history browsing is a future enhancement.

## Key Bindings

| Key | Context | Action |
|-----|---------|--------|
| `/` or `s` | Global | Focus search bar |
| `ESC` | Search/Detail | Go back |
| `1` `2` `3` | Home | Switch tab (For You / Subscriptions / History) |
| `h` `j` `k` `l` | Card grid | Navigate left/down/up/right |
| `j` `k` | Lists | Navigate up/down |
| `Enter` | Any | Select / confirm |
| `Space` | Global | Toggle pause |
| `>` / `<` | Global | Seek forward/back 10s |
| `+` / `-` | Global | Volume up/down |
| `q` | Global | Quit |
| `?` | Global | Show help |
| `:` | Global | Command mode |

## File Structure

```
youtube-terminal/
├── Cargo.toml
├── src/
│   ├── main.rs              — entry point, terminal setup/teardown
│   ├── app.rs               — AppState, Action enum, dispatch logic
│   ├── event.rs             — event loop, crossterm polling, async result draining
│   ├── auth/
│   │   ├── mod.rs           — AuthBackend trait, AuthCapabilities, AuthState
│   │   └── cookies.rs       — Netscape cookie file parsing + validation
│   ├── provider/
│   │   ├── mod.rs           — ContentProvider trait, FeedPage, FeedItem, data models
│   │   └── rustypipe.rs     — RustyPipe implementation (with userdata feature)
│   ├── player/
│   │   ├── mod.rs           — Player trait, PlayerState, PlayerInfo
│   │   └── mpv.rs           — mpv process lifecycle + JSON IPC client
│   ├── ui/
│   │   ├── mod.rs           — root layout, render dispatch
│   │   ├── search_bar.rs    — search input widget
│   │   ├── tab_bar.rs       — tab navigation widget
│   │   ├── card_grid.rs     — responsive video card grid with ASCII thumbnails
│   │   ├── video_list.rs    — compact search results list
│   │   ├── video_detail.rs  — detail view with action menu
│   │   └── now_playing.rs   — bottom playback bar with progress
│   ├── db/
│   │   ├── mod.rs           — SQLite connection setup + migrations
│   │   ├── history.rs       — local watch history queries
│   │   └── cache.rs         — thumbnail index + metadata cache
│   └── config.rs            — TOML config loading + defaults
├── docs/
│   └── plans/
│       └── 2026-03-05-youtube-terminal-design.md
└── README.md
```

## Dependencies (Cargo.toml)

```toml
[package]
name = "youtube-terminal"
version = "0.1.0"
edition = "2021"
license = "GPL-3.0-or-later"

[dependencies]
ratatui = "0.29"
crossterm = "0.29"
tokio = { version = "1", features = ["full"] }
rustypipe = { version = "0.11", features = ["userdata"] }
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
image = "0.25"
chrono = { version = "0.4", features = ["serde"] }
dirs = "5"
anyhow = "1"
async-trait = "0.1"
```

## Non-Goals (for v1)

- Video rendering inside the terminal
- Sixel/Kitty image protocol support
- Comment section
- Live chat
- Upload functionality
- YouTube Music integration
- Multiple account support
- Queue / playlist management (requires designed PlayerState)
- Browser-native cookie extraction
- Remote history sync / write-back
- Personalized "For You" if raw InnerTube proves too fragile (degrade to Trending)

## Resolved Design Decisions

1. **Infinite scroll with continuation tokens** — feeds load the next page when the user scrolls near the bottom. No explicit pagination UI.
2. **Keyboard-only for v1** — mouse support deferred. Alacritty + tmux mouse handling adds complexity.
3. **Single mpv process** — no queue. Play a video, it replaces whatever was playing. Queue is a v2 feature.
4. **GPL-3.0 license** — required by RustyPipe dependency. Acceptable for a personal tool.
5. **Unix-only for v1** — IPC sockets, file permissions, XDG paths. Windows support is not a goal.
