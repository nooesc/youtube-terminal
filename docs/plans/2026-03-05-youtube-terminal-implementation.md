# youtube-terminal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a terminal-based YouTube client with search, subscription feed, local history, and mpv playback.

**Architecture:** Async TUI app using ratatui for rendering, RustyPipe for YouTube data, mpv via JSON IPC for playback, SQLite for persistence. Background tokio tasks communicate with the UI thread via mpsc channels. Single AppState struct with Action-based dispatch.

**Tech Stack:** Rust 1.93+, ratatui 0.29, crossterm 0.29, tokio, rustypipe 0.11 (userdata feature), rusqlite, mpv (external), yt-dlp (external)

**Design Doc:** `docs/plans/2026-03-05-youtube-terminal-design.md`

---

## Prerequisites

Before starting, install external dependencies:

```bash
brew install mpv yt-dlp
```

## Phase 1: Project Skeleton + Data Models

### Task 1: Initialize Cargo project and git repo

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

**Step 1: Init project**

```bash
cd /Users/coler/dev-personal/youtube-terminal
cargo init --name youtube-terminal
git init
```

**Step 2: Create .gitignore**

```
/target
*.swp
*.swo
.DS_Store
```

**Step 3: Set up Cargo.toml with all dependencies**

Replace `Cargo.toml` with:

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

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors (warnings OK)

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: initialize cargo project with dependencies"
```

---

### Task 2: Data models

**Files:**
- Create: `src/models.rs`
- Modify: `src/main.rs` (add module declaration)

**Step 1: Write data model types**

Create `src/models.rs` with all shared types from the design doc:
- `FeedPage<T>`, `FeedItem`, `VideoItem`, `VideoDetail`, `ChannelItem`, `PlaylistItem`, `HistoryEntry`
- `ThumbnailKey`, `ItemType`
- `SearchFilters` (empty struct for now, fields added later)
- Derive `Clone`, `Debug`, `serde::Serialize`, `serde::Deserialize` where appropriate

**Step 2: Add module to main.rs**

```rust
mod models;

fn main() {
    println!("youtube-terminal v0.1.0");
}
```

**Step 3: Verify it compiles**

Run: `cargo check`

**Step 4: Commit**

```bash
git add src/models.rs src/main.rs
git commit -m "feat: add core data models"
```

---

### Task 3: Config module

**Files:**
- Create: `src/config.rs`

**Step 1: Write config struct with TOML loading**

```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub cookie_file: Option<PathBuf>,
    pub mpv_geometry: String,
    pub mpv_ontop: bool,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("youtube-terminal");
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("~/.cache"))
            .join("youtube-terminal");
        Self {
            cookie_file: None,
            mpv_geometry: "400x225+0+0".to_string(),
            mpv_ontop: true,
            data_dir,
            cache_dir,
        }
    }
}
```

Add `Config::load()` that reads from `~/.config/youtube-terminal/config.toml` with fallback to defaults.

**Step 2: Verify it compiles**

Run: `cargo check`

**Step 3: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add config module with TOML loading"
```

---

## Phase 2: Database Layer

### Task 4: SQLite setup + migrations

**Files:**
- Create: `src/db/mod.rs`
- Create: `src/db/history.rs`
- Create: `src/db/cache.rs`

**Step 1: Write DB initialization with schema creation**

`src/db/mod.rs`:
- `Database` struct wrapping `rusqlite::Connection`
- `Database::open(path)` that creates the file + runs migrations
- Schema: `watch_history`, `metadata_cache`, `thumbnail_index`, `settings` tables

**Step 2: Write history module**

`src/db/history.rs`:
- `add_to_history(video_id, title, channel, channel_id, thumbnail_url)`
- `get_history(limit, offset) -> Vec<HistoryEntry>`
- `clear_history()`

**Step 3: Write cache module**

`src/db/cache.rs`:
- `get_cached_metadata(video_id) -> Option<String>` (JSON string, checks TTL)
- `set_cached_metadata(video_id, json_data)`
- `get_thumbnail_path(key: &ThumbnailKey) -> Option<PathBuf>`
- `set_thumbnail_path(key: &ThumbnailKey, path: &Path)`
- `cleanup_old_thumbnails(max_age_days, max_count)`

**Step 4: Write tests**

Test: history insert + retrieve, cache TTL expiry, thumbnail index CRUD.

Run: `cargo test -- db`
Expected: all pass

**Step 5: Commit**

```bash
git add src/db/
git commit -m "feat: add SQLite database layer with history and cache"
```

---

## Phase 3: Provider Layer (RustyPipe)

### Task 5: ContentProvider trait + RustyPipe implementation

**Files:**
- Create: `src/provider/mod.rs`
- Create: `src/provider/rustypipe.rs`

**Step 1: Define ContentProvider trait**

`src/provider/mod.rs`:
- `ContentProvider` trait with all methods from design doc
- `AuthCapabilities` struct
- Re-export data models

**Step 2: Implement RustyPipe provider**

`src/provider/rustypipe.rs`:
- `RustyPipeProvider` struct holding `RustyPipe` client instance
- `RustyPipeProvider::new(storage_dir)` — creates client with storage dir for cache
- `RustyPipeProvider::set_cookies(cookie_txt: &str)` — feeds cookies via `user_auth_set_cookie_txt`
- Implement `ContentProvider` for `RustyPipeProvider`:
  - `search()` → `query.search_filter()` or `query.search()`, map results to `FeedPage<FeedItem>`
  - `trending()` → `query.trending()`, map to `FeedPage<VideoItem>`
  - `video_detail()` → `query.video_details()`, map to `VideoDetail`
  - `channel()` → `query.channel_info()`, map to `ChannelDetail`
  - `channel_videos()` → `query.channel_videos()`, map to `FeedPage<VideoItem>`
  - Auth methods → use `query.authenticated()` variants
  - `capabilities()` → return flags based on whether cookies are loaded

**Note:** This task requires careful mapping between RustyPipe types and our models. Consult `docs.rs/rustypipe` for exact type signatures. Some methods may need `tokio::runtime::Handle` if RustyPipe is sync-only.

**Step 3: Write a basic integration test**

Test that `RustyPipeProvider::new()` creates successfully and `search("rust programming", ...)` returns results (requires network).

Run: `cargo test -- provider --ignored` (mark network tests as `#[ignore]`)

**Step 4: Commit**

```bash
git add src/provider/
git commit -m "feat: add ContentProvider trait and RustyPipe implementation"
```

---

## Phase 4: Player Layer (mpv IPC)

### Task 6: mpv JSON IPC client

**Files:**
- Create: `src/player/mod.rs`
- Create: `src/player/mpv.rs`

**Step 1: Define Player trait and types**

`src/player/mod.rs`:
- `PlayerState` enum: `Stopped`, `Playing(PlayerInfo)`, `Paused(PlayerInfo)`
- `PlayerInfo` struct: `title: String`, `time_pos: f64`, `duration: f64`, `volume: f64`
- `PlayMode` enum: `Video`, `AudioOnly`

**Step 2: Implement MpvPlayer**

`src/player/mpv.rs`:
- `MpvPlayer` struct with: `socket_path: PathBuf`, `process: Option<Child>`, `stream: Option<UnixStream>`
- `MpvPlayer::new()` — generates socket path `/tmp/yt-term-{pid}.sock`, cleans stale sockets
- `play(url: &str, mode: PlayMode, cookie_path: Option<&Path>)`:
  - Kill existing mpv process if any
  - Spawn `mpv` with appropriate flags (video vs audio-only)
  - Connect to IPC socket (retry with small delay for mpv startup)
- `send_command(cmd: &[&str]) -> Result<serde_json::Value>`:
  - Write JSON command to socket, read response
- `get_property(name: &str) -> Result<serde_json::Value>`
- `set_property(name: &str, value: serde_json::Value)`
- `toggle_pause()`, `seek(seconds: f64)`, `set_volume(vol: f64)`
- `poll_state() -> Result<PlayerState>` — reads time-pos, duration, media-title
- `stop()` — send quit command, kill process, clean up socket
- `Drop` impl — cleanup on drop

**Step 3: Write test**

Test `MpvPlayer::new()` creates correct socket path. Test command serialization. (Playback tests require mpv installed — mark `#[ignore]`.)

Run: `cargo test -- player`

**Step 4: Commit**

```bash
git add src/player/
git commit -m "feat: add mpv JSON IPC player implementation"
```

---

## Phase 5: Auth Layer

### Task 7: Cookie import + auth state

**Files:**
- Create: `src/auth/mod.rs`
- Create: `src/auth/cookies.rs`

**Step 1: Write cookie parsing**

`src/auth/cookies.rs`:
- `parse_netscape_cookies(content: &str) -> Result<Vec<Cookie>>` — parse Netscape cookie.txt format
- `import_cookie_file(source: &Path, dest: &Path) -> Result<()>` — copy file, set permissions 0600
- `validate_cookies(dest: &Path) -> bool` — check file exists, non-empty, readable

**Step 2: Write auth state**

`src/auth/mod.rs`:
- `AuthState` enum: `NoAuth`, `Authenticated { cookie_path: PathBuf }`
- `AuthState::load(config: &Config) -> AuthState` — check if cookie file exists at expected location
- `AuthState::cookie_path(&self) -> Option<&Path>`
- `AuthCapabilities` — derived from AuthState

**Step 3: Write tests**

Test Netscape cookie file parsing with sample data. Test import copies file correctly.

Run: `cargo test -- auth`

**Step 4: Commit**

```bash
git add src/auth/
git commit -m "feat: add cookie import and auth state"
```

---

## Phase 6: App State + Event Loop

### Task 8: AppState and Action dispatch

**Files:**
- Create: `src/app.rs`

**Step 1: Define AppState**

All types from design doc: `AppState`, `View`, `Action`, `LoadingState`, `LoadedPage`, `Tab`, `Direction`, `TabState`, `SearchState`, `CardGridState`, `VideoListState`, `DetailState`.

**Step 2: Implement dispatch**

`AppState::dispatch(&mut self, action: Action)` — match on Action enum, update state accordingly:
- `SwitchTab` → change active tab, trigger feed load
- `Navigate` → move selection in current view
- `Select` → based on context, load detail or play
- `Back` → pop previous view
- `FocusSearch` → switch to Search view
- `SubmitSearch` → trigger search request
- `FeedLoaded` → check request_id, store results
- `SearchResults` → check request_id, store results
- `ThumbnailReady` → mark thumbnail as cached
- `PlayerStateUpdate` → update player info
- Playback actions → delegate to player

**Step 3: Write tests**

Test state transitions: `SwitchTab` changes tab, `Navigate` moves cursor, `Back` pops view stack, stale request_id is ignored.

Run: `cargo test -- app`

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: add AppState with Action dispatch"
```

---

### Task 9: Event loop

**Files:**
- Create: `src/event.rs`

**Step 1: Write event loop**

`src/event.rs`:
- `EventLoop` struct holding: `result_rx: mpsc::UnboundedReceiver<Action>`, `result_tx: mpsc::UnboundedSender<Action>`
- `EventLoop::new() -> (EventLoop, ActionSender)` — create channels
- `EventLoop::run(terminal, state, provider, player, db)`:
  - Main loop: poll crossterm events (100ms), drain mpsc channel, dispatch actions, render
  - Map key events to Actions using keybinding logic
  - Handle resize events

**Step 2: Write keybinding mapper**

`fn map_key_event(key: KeyEvent, view: &View) -> Option<Action>`:
- `/` or `s` → `FocusSearch`
- `1`/`2`/`3` → `SwitchTab(ForYou/Subscriptions/History)`
- `h`/`j`/`k`/`l` or arrow keys → `Navigate(Direction)`
- `Enter` → `Select`
- `ESC` → `Back`
- `Space` → `TogglePause`
- `>`/`<` → `Seek(10.0)` / `Seek(-10.0)`
- `+`/`-` → `VolumeUp` / `VolumeDown`
- `q` → `Quit`

**Step 3: Verify it compiles**

Run: `cargo check`

**Step 4: Commit**

```bash
git add src/event.rs
git commit -m "feat: add event loop with keybinding mapper"
```

---

### Task 10: Wire up main.rs

**Files:**
- Modify: `src/main.rs`

**Step 1: Write main function**

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Load config
    // 2. Init database
    // 3. Init auth state
    // 4. Init RustyPipe provider (with cookies if available)
    // 5. Init MpvPlayer
    // 6. Init AppState
    // 7. Setup terminal (crossterm raw mode, alternate screen)
    // 8. Create event loop channels
    // 9. Spawn initial feed load as background task
    // 10. Run event loop
    // 11. Restore terminal on exit
}
```

**Step 2: Verify it compiles and runs**

Run: `cargo run`
Expected: terminal enters alternate screen, shows placeholder UI, `q` exits cleanly.

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up main with terminal setup and event loop"
```

---

## Phase 7: UI Components

### Task 11: Root layout + tab bar

**Files:**
- Create: `src/ui/mod.rs`
- Create: `src/ui/tab_bar.rs`

**Step 1: Write root layout**

`src/ui/mod.rs`:
- `pub fn render(f: &mut Frame, state: &AppState)`:
  - Split terminal into: search bar (3 rows) | tab bar (1 row) | main content (fill) | now-playing bar (3 rows)
  - Dispatch to sub-renderers based on `state.view`

**Step 2: Write tab bar widget**

`src/ui/tab_bar.rs`:
- Render `For You | Subscriptions | History` with active tab highlighted
- Use ratatui `Tabs` widget

**Step 3: Verify it renders**

Run: `cargo run`
Expected: see search bar placeholder, tab bar with 3 tabs, empty content area, empty now-playing bar.

**Step 4: Commit**

```bash
git add src/ui/
git commit -m "feat: add root layout and tab bar"
```

---

### Task 12: Search bar widget

**Files:**
- Create: `src/ui/search_bar.rs`

**Step 1: Write search bar**

- Render `/ Search...` placeholder when unfocused
- Render `/ {user_input}_` with cursor when focused
- Handle text input: append chars, backspace, clear
- `SearchState` tracks: `query: String`, `cursor: usize`, `focused: bool`

**Step 2: Add text input handling to event loop**

When search bar is focused, key events go to text input instead of normal keybindings. `Enter` submits, `ESC` unfocuses.

**Step 3: Verify it works**

Run: `cargo run`
Expected: press `/`, type text, see it appear, press `ESC` to exit.

**Step 4: Commit**

```bash
git add src/ui/search_bar.rs src/event.rs src/app.rs
git commit -m "feat: add interactive search bar"
```

---

### Task 13: Video list widget (search results)

**Files:**
- Create: `src/ui/video_list.rs`

**Step 1: Write video list renderer**

- Render list of `FeedItem` entries as compact rows
- Format: `▸ Title — Channel          Views · Age`
- Highlight selected item
- Show loading spinner when `search_loading` is true
- Show "No results" when list is empty and not loading

**Step 2: Wire search submission to RustyPipe**

When `SubmitSearch(query)` is dispatched:
- Set `search_loading = true`, increment `search_request_id`
- Spawn background task: `provider.search(query, filters, None)`
- Send `SearchResults(request_id, result)` back via channel

**Step 3: Verify end-to-end search works**

Run: `cargo run`
Expected: press `/`, type query, press Enter, see loading state, then real YouTube results appear. Navigate with j/k.

**Step 4: Commit**

```bash
git add src/ui/video_list.rs src/app.rs src/event.rs
git commit -m "feat: add search results list with live YouTube search"
```

---

### Task 14: Video detail view

**Files:**
- Create: `src/ui/video_detail.rs`

**Step 1: Write detail renderer**

- Show: title, channel, view count, upload date, description (scrollable)
- Action menu: Play Video, Play Audio Only, Open Channel, Download
- j/k navigates action menu, Enter executes selected action

**Step 2: Wire detail loading**

When user selects a video from search results or card grid:
- Set `detail_loading = true`
- Spawn: `provider.video_detail(id)`
- On result: store in `state.detail`, switch to `View::VideoDetail(id)`

**Step 3: Verify it works**

Run: `cargo run`
Expected: search for something, press Enter on a result, see detail view with metadata and actions.

**Step 4: Commit**

```bash
git add src/ui/video_detail.rs src/app.rs
git commit -m "feat: add video detail view with action menu"
```

---

### Task 15: Now-playing bar

**Files:**
- Create: `src/ui/now_playing.rs`

**Step 1: Write now-playing renderer**

- When `PlayerState::Stopped`: show empty bar or "No media playing"
- When `Playing`/`Paused`: show `▶/❚❚ Title — Channel  ├──────┤ 2:30/4:15`
- Progress bar using Unicode block characters
- Format time as `M:SS`

**Step 2: Spawn mpv polling task**

Background task that polls mpv IPC every 1 second:
- `player.poll_state()`
- Send `PlayerStateUpdate(info)` via channel

**Step 3: Verify it works**

This will be testable once playback works (next task).

**Step 4: Commit**

```bash
git add src/ui/now_playing.rs
git commit -m "feat: add now-playing bar with progress display"
```

---

## Phase 8: Playback Integration

### Task 16: Connect video detail actions to mpv

**Files:**
- Modify: `src/app.rs`
- Modify: `src/event.rs`

**Step 1: Handle PlayVideo and PlayAudio actions**

When dispatched:
- Get video URL from current detail state: `https://www.youtube.com/watch?v={id}`
- Call `player.play(url, mode, cookie_path)`
- Add entry to local watch history via DB
- Start mpv polling background task if not already running

**Step 2: Handle playback control actions**

- `TogglePause` → `player.toggle_pause()`
- `Seek(secs)` → `player.seek(secs)`
- `VolumeUp/Down` → `player.set_volume(current ± 5)`

**Step 3: Test end-to-end playback**

Run: `cargo run`
Expected: search → select video → "Play Video" opens mpv window with video. "Play Audio Only" plays audio in background. Now-playing bar shows progress. Space pauses/resumes.

**Step 4: Commit**

```bash
git add src/app.rs src/event.rs
git commit -m "feat: connect playback actions to mpv"
```

---

## Phase 9: Card Grid + Homepage

### Task 17: Card grid widget

**Files:**
- Create: `src/ui/card_grid.rs`

**Step 1: Write card grid renderer**

- Calculate number of columns from terminal width (each card ~20-25 chars wide)
- Render cards as: thumbnail placeholder (can be colored box initially) | title | channel | stats
- 2D navigation: h/l across columns, j/k across rows
- Highlight selected card with border color change
- `CardGridState` tracks: `selected_row`, `selected_col`, `columns`, `items: Vec<FeedItem>`

**Step 2: Wire to AppState navigation**

`Navigate(Direction)` in Home view → move card selection. Handle wrapping at row/column boundaries.

**Step 3: Verify it renders**

Run: `cargo run`
Expected: homepage shows card grid with placeholder cards. hjkl navigates between them.

**Step 4: Commit**

```bash
git add src/ui/card_grid.rs src/app.rs
git commit -m "feat: add responsive card grid widget"
```

---

### Task 18: Load trending/home feed into card grid

**Files:**
- Modify: `src/app.rs`
- Modify: `src/event.rs`

**Step 1: Load initial feed on startup**

On app start:
- If authenticated: attempt `provider.home_feed(None)` (may fall back to trending)
- If not authenticated: `provider.trending()`
- Send `FeedLoaded(request_id, result)` via channel

**Step 2: Handle FeedLoaded for home tab**

Store items in `CardGridState`, clear loading state.

**Step 3: Handle tab switching**

- Tab 1 (For You): load home feed or trending
- Tab 2 (Subscriptions): load `subscription_feed()` if authenticated, else show message
- Tab 3 (History): load from local SQLite

**Step 4: Verify it works**

Run: `cargo run`
Expected: app starts, shows loading, then trending videos appear as cards. Can switch tabs.

**Step 5: Commit**

```bash
git add src/app.rs src/event.rs
git commit -m "feat: load trending/home feed into card grid"
```

---

## Phase 10: Thumbnail Rendering

### Task 19: Thumbnail download + half-block rendering

**Files:**
- Create: `src/thumbnails.rs`
- Modify: `src/ui/card_grid.rs`

**Step 1: Write thumbnail downloader**

`src/thumbnails.rs`:
- `download_thumbnail(url: &str, key: &ThumbnailKey, cache_dir: &Path) -> Result<PathBuf>`
  - Download image to `{cache_dir}/thumbs/{item_type}_{item_id}.jpg`
  - Resize to card dimensions (e.g. 40x20 pixels for half-block at 20x10 cells)
- Use `image` crate for decode + resize

**Step 2: Write half-block renderer**

- `render_halfblock(img: &DynamicImage, area: Rect, buf: &mut Buffer)`
- For each pair of vertical pixels, use `▀` character with top pixel as foreground color, bottom as background color
- This gives 2x vertical resolution

**Step 3: Integrate into card grid**

- When card grid receives items, spawn thumbnail download tasks (max 4 concurrent via semaphore)
- On `ThumbnailReady`, re-render affected card with real thumbnail
- Show colored placeholder while downloading

**Step 4: Verify thumbnails appear**

Run: `cargo run`
Expected: cards initially show placeholders, then thumbnails fade in as they download. Half-block art shows recognizable (if low-res) thumbnails.

**Step 5: Commit**

```bash
git add src/thumbnails.rs src/ui/card_grid.rs
git commit -m "feat: add thumbnail download and half-block ASCII rendering"
```

---

## Phase 11: History + Polish

### Task 20: Local watch history

**Files:**
- Modify: `src/app.rs`

**Step 1: Record history on playback**

When `PlayVideo` or `PlayAudio` action fires, insert into `watch_history` table.

**Step 2: Load history in History tab**

When History tab is selected, query local DB and display in card grid.

**Step 3: Verify**

Run: `cargo run`
Expected: play a video, switch to History tab, see it there.

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: record and display local watch history"
```

---

### Task 21: Infinite scroll (continuation tokens)

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui/card_grid.rs`
- Modify: `src/ui/video_list.rs`

**Step 1: Detect scroll-near-bottom**

In card grid and video list: when cursor moves to the last 2 items and there's a continuation token, trigger next page load.

**Step 2: Append results**

`FeedLoaded` and `SearchResults` with continuation: append items to existing list, update continuation token.

**Step 3: Verify**

Run: `cargo run`
Expected: scroll to bottom of search results or card grid, see more items load automatically.

**Step 4: Commit**

```bash
git add src/app.rs src/ui/card_grid.rs src/ui/video_list.rs
git commit -m "feat: add infinite scroll with continuation tokens"
```

---

### Task 22: Cookie import command

**Files:**
- Modify: `src/event.rs`
- Modify: `src/app.rs`

**Step 1: Add command mode**

When user presses `:`, enter command mode (text input at bottom of screen). Support:
- `:import-cookies /path/to/cookies.txt` — import cookie file
- `:q` — quit

**Step 2: Handle import-cookies**

Parse command, call `import_cookie_file()`, reload provider with cookies, refresh feed.

**Step 3: Verify**

Run: `cargo run`
Expected: `:import-cookies ~/cookies.txt` imports the file, shows confirmation, and refreshes with authenticated data.

**Step 4: Commit**

```bash
git add src/event.rs src/app.rs
git commit -m "feat: add command mode with cookie import"
```

---

### Task 23: Graceful degradation + error handling

**Files:**
- Modify: various

**Step 1: Handle network errors**

Show error messages in UI (e.g. "Network error — press r to retry") instead of crashing.

**Step 2: Handle auth failures**

When authenticated calls fail with 401/403, switch to no-auth mode and show message.

**Step 3: Handle mpv not found**

On playback attempt, if mpv binary not found, show "mpv not installed" message.

**Step 4: Clean terminal restore on panic**

Use `std::panic::set_hook` to restore terminal state before printing panic info.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add error handling and graceful degradation"
```

---

### Task 24: Final cleanup + README

**Files:**
- Modify: `src/main.rs` (any remaining module wiring)
- Create: `README.md`

**Step 1: Write README**

- Project description
- Screenshots / usage GIF placeholder
- Installation: `cargo install --path .`
- Prerequisites: mpv, yt-dlp
- Setup: how to export cookies from Firefox
- Key bindings reference
- License: GPL-3.0

**Step 2: Cargo clippy + format**

```bash
cargo clippy -- -D warnings
cargo fmt
```

**Step 3: Final commit**

```bash
git add -A
git commit -m "docs: add README and final cleanup"
```

---

## Task Dependency Graph

```
Phase 1: [Task 1] → [Task 2] → [Task 3]
Phase 2: [Task 4]
Phase 3: [Task 5]
Phase 4: [Task 6]
Phase 5: [Task 7]
Phase 6: [Task 8] → [Task 9] → [Task 10]  (depends on all above)
Phase 7: [Task 11] → [Task 12] → [Task 13] → [Task 14] → [Task 15]
Phase 8: [Task 16]  (depends on Task 14 + Task 6)
Phase 9: [Task 17] → [Task 18]  (depends on Task 11 + Task 5)
Phase 10: [Task 19]  (depends on Task 17)
Phase 11: [Task 20] → [Task 21] → [Task 22] → [Task 23] → [Task 24]
```

**Phases 2-5 can be built in parallel** — they have no dependencies on each other. Phase 6 depends on all of them. Phase 7 can partially overlap with Phase 8.
