# youtube-terminal

A terminal-based YouTube client built with Rust. Browse trending videos, search
YouTube, manage subscriptions, and play content through mpv -- all from your
terminal.

Built with [ratatui](https://ratatui.rs) for the TUI,
[mpv](https://mpv.io) for playback, and
[RustyPipe](https://codeberg.org/ThetaDev/rustypipe) for YouTube data.

## Prerequisites

- **Rust toolchain** (1.73+) -- install via [rustup](https://rustup.rs)
- **mpv** -- media player for video/audio playback
- **yt-dlp** -- used by mpv to resolve YouTube URLs

### Install prerequisites (macOS)

```sh
brew install mpv yt-dlp
```

### Install prerequisites (Linux)

```sh
# Debian/Ubuntu
sudo apt install mpv yt-dlp

# Arch
sudo pacman -S mpv yt-dlp
```

## Installation

```sh
cargo install --path .
```

Or build and run directly:

```sh
cargo run --release
```

## Cookie setup

To access personalized feeds (home feed, subscriptions), you need to export
your YouTube cookies from your browser.

### Export cookies from Firefox

1. Install the [cookies.txt](https://addons.mozilla.org/en-US/firefox/addon/cookies-txt/)
   Firefox extension.
2. Go to [youtube.com](https://www.youtube.com) and make sure you are logged in.
3. Click the extension icon and export cookies for `youtube.com`.
4. Save the file (e.g., `~/cookies.txt`).

### Import cookies

From within youtube-terminal, press `:` to enter command mode, then type:

```
import-cookies ~/cookies.txt
```

The cookies are copied to the application data directory with restricted
permissions (0600).

## Key bindings

| Key                        | Action                    |
| -------------------------- | ------------------------- |
| `q` / `Ctrl+c`            | Quit                      |
| `/` or `s`                 | Focus search              |
| `1` / `2` / `3`           | Switch tab (For You / Subscriptions / History) |
| `h` `j` `k` `l` / arrows  | Navigate                  |
| `Enter`                    | Select                    |
| `Esc`                      | Back                      |
| `Space`                    | Toggle pause              |
| `<` / `>`                  | Seek -10s / +10s          |
| `+` / `=` / `-`            | Volume up / down          |
| `:`                        | Command mode              |

### Commands

| Command                    | Description               |
| -------------------------- | ------------------------- |
| `:q`                       | Quit                      |
| `:import-cookies <path>`   | Import browser cookies    |

## License

GPL-3.0-or-later
