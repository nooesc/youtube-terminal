use super::{PlayMode, PlaybackQuality, PlayerInfo, PlayerState};
use crate::config::Config;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

fn send_command_on_stream(stream: &mut UnixStream, cmd: &[Value]) -> Result<Option<Value>> {
    let msg = json!({"command": cmd});
    let mut line = serde_json::to_string(&msg)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let resp: Value = serde_json::from_str(&response_line)?;
    if resp.get("error").and_then(|e| e.as_str()) == Some("success") {
        Ok(resp.get("data").cloned())
    } else {
        let err = resp
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown error");
        bail!("mpv command error: {}", err)
    }
}

fn build_launch_args(
    socket_path: &Path,
    url: &str,
    mode: PlayMode,
    quality: PlaybackQuality,
    config: &Config,
    geometry_override: Option<&str>,
    log_path: &Path,
    cookie_path: Option<&Path>,
) -> Vec<String> {
    let force_seekable = if config.mpv_force_seekable {
        "yes"
    } else {
        "no"
    };
    let mut args = vec![
        format!("--input-ipc-server={}", socket_path.display()),
        format!("--log-file={}", log_path.display()),
        "--cache=yes".to_string(),
        "--cache-pause=yes".to_string(),
        format!("--hwdec={}", config.mpv_hwdec),
        format!("--cache-secs={}", config.mpv_cache_secs),
        format!("--cache-pause-wait={}", config.mpv_cache_pause_wait),
        format!("--force-seekable={force_seekable}"),
        format!("--demuxer-max-bytes={}", config.mpv_demuxer_max_bytes),
        format!(
            "--demuxer-max-back-bytes={}",
            config.mpv_demuxer_max_back_bytes
        ),
        format!("--ytdl-format={}", quality.ytdl_format()),
    ];

    match mode {
        PlayMode::Video => {
            let geometry = geometry_override.unwrap_or(&config.mpv_geometry);
            args.push(format!("--geometry={geometry}"));
            if config.mpv_ontop {
                args.push("--ontop".to_string());
            }

            // macOS native appearance
            if cfg!(target_os = "macos") {
                args.extend([
                    "--macos-title-bar-appearance=darkAqua".to_string(),
                    "--macos-title-bar-material=dark".to_string(),
                    "--corner-rounding=0.5".to_string(),
                    "--background=color".to_string(),
                    "--background-color=#000000".to_string(),
                    "--osd-font=SF Pro".to_string(),
                    "--osd-font-size=32".to_string(),
                    "--osd-color=#FFFFFFFF".to_string(),
                    "--osd-outline-size=1".to_string(),
                    "--osd-outline-color=#00000080".to_string(),
                    "--osd-shadow-offset=0".to_string(),
                    "--osd-bar-h=2".to_string(),
                    "--osd-bar-w=90".to_string(),
                    "--osd-bar-align-y=0.95".to_string(),
                ]);
            }
        }
        PlayMode::AudioOnly => {
            args.push("--no-video".to_string());
        }
    }

    if let Some(cookie) = cookie_path {
        if cookie.exists() {
            args.push(format!("--ytdl-raw-options=cookies={}", cookie.display()));
        }
    }

    args.push(url.to_string());
    args
}

pub fn poll_socket_state(socket_path: &Path) -> PlayerState {
    if !socket_path.exists() {
        return PlayerState::Stopped;
    }

    let mut stream = match UnixStream::connect(socket_path) {
        Ok(stream) => stream,
        Err(_) => return PlayerState::Stopped,
    };

    if stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .is_err()
    {
        return PlayerState::Stopped;
    }

    let paused = send_command_on_stream(&mut stream, &[json!("get_property"), json!("pause")])
        .ok()
        .flatten()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let time_pos = send_command_on_stream(&mut stream, &[json!("get_property"), json!("time-pos")])
        .ok()
        .flatten()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let duration = send_command_on_stream(&mut stream, &[json!("get_property"), json!("duration")])
        .ok()
        .flatten()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let title = send_command_on_stream(&mut stream, &[json!("get_property"), json!("media-title")])
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(ToString::to_string))
        .unwrap_or_default();

    let volume = send_command_on_stream(&mut stream, &[json!("get_property"), json!("volume")])
        .ok()
        .flatten()
        .and_then(|v| v.as_f64())
        .unwrap_or(100.0);

    let info = PlayerInfo {
        title,
        time_pos,
        duration,
        volume,
    };

    if paused {
        PlayerState::Paused(info)
    } else {
        PlayerState::Playing(info)
    }
}

pub struct MpvPlayer {
    socket_path: PathBuf,
    process: Option<Child>,
    stream: Option<UnixStream>,
    owns_process: bool,
}

#[allow(dead_code)]
impl MpvPlayer {
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            process: None,
            stream: None,
            owns_process: false,
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn attach_if_running(&mut self) -> bool {
        if !self.socket_path.exists() {
            return false;
        }
        self.process = None;
        self.stream = None;
        self.owns_process = false;
        self.ensure_connected().is_ok()
    }

    pub fn play(
        &mut self,
        url: &str,
        mode: PlayMode,
        quality: PlaybackQuality,
        config: &Config,
        geometry_override: Option<&str>,
        cookie_path: Option<&Path>,
    ) -> Result<()> {
        // Kill existing process if any
        self.stop();
        let _ = std::fs::remove_file(&self.socket_path);

        let log_path = config.mpv_log_path();
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut cmd = Command::new("mpv");
        cmd.stdin(std::process::Stdio::null());
        cmd.args(build_launch_args(
            &self.socket_path,
            url,
            mode,
            quality,
            config,
            geometry_override,
            &log_path,
            cookie_path,
        ));
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = cmd
            .spawn()
            .context("failed to spawn mpv — is it installed?")?;
        self.process = Some(child);
        self.stream = None;
        self.owns_process = true;

        Ok(())
    }

    fn connect_with_retry(&mut self) -> Result<()> {
        for i in 0..20 {
            if self.socket_path.exists() {
                match UnixStream::connect(&self.socket_path) {
                    Ok(stream) => {
                        stream.set_read_timeout(Some(Duration::from_millis(250)))?;
                        self.stream = Some(stream);
                        return Ok(());
                    }
                    Err(_) if i < 19 => {}
                    Err(e) => return Err(e.into()),
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        bail!("timed out waiting for mpv socket")
    }

    fn ensure_connected(&mut self) -> Result<()> {
        if self.stream.is_none() {
            self.connect_with_retry()?;
        }
        Ok(())
    }

    pub fn send_command(&mut self, cmd: &[Value]) -> Result<Option<Value>> {
        self.ensure_connected()?;
        let stream = self.stream.as_mut().context("mpv not connected")?;
        send_command_on_stream(stream, cmd)
    }

    pub fn get_property(&mut self, name: &str) -> Result<Value> {
        self.send_command(&[json!("get_property"), json!(name)])?
            .context("no data returned for property")
    }

    pub fn set_property(&mut self, name: &str, value: Value) -> Result<()> {
        self.send_command(&[json!("set_property"), json!(name), value])?;
        Ok(())
    }

    pub fn toggle_pause(&mut self) -> Result<()> {
        let paused = self.get_property("pause")?.as_bool().unwrap_or(false);
        self.set_property("pause", json!(!paused))
    }

    pub fn seek(&mut self, seconds: f64) -> Result<()> {
        self.send_command(&[json!("seek"), json!(seconds), json!("relative+exact")])?;
        Ok(())
    }

    pub fn seek_to(&mut self, seconds: f64) -> Result<()> {
        self.send_command(&[json!("seek"), json!(seconds), json!("absolute+exact")])?;
        Ok(())
    }

    pub fn window_geometry(&mut self) -> Result<String> {
        self.get_property("geometry")?
            .as_str()
            .map(str::to_string)
            .context("mpv geometry property unavailable")
    }

    pub fn set_volume(&mut self, vol: f64) -> Result<()> {
        self.set_property("volume", json!(vol))
    }

    pub fn poll_state(&mut self) -> Result<PlayerState> {
        // Check if process is still alive
        if let Some(ref mut child) = self.process {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process exited
                    self.cleanup();
                    return Ok(PlayerState::Stopped);
                }
                Ok(None) => {} // still running
                Err(_) => {
                    self.cleanup();
                    return Ok(PlayerState::Stopped);
                }
            }
        }

        if !self.socket_path.exists() {
            self.stream = None;
            return Ok(PlayerState::Stopped);
        }

        if self.stream.is_none() && self.ensure_connected().is_err() {
            return Ok(PlayerState::Stopped);
        }

        let paused = self
            .get_property("pause")
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let time_pos = self
            .get_property("time-pos")
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let duration = self
            .get_property("duration")
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let title = self
            .get_property("media-title")
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        let volume = self
            .get_property("volume")
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(100.0);

        let info = PlayerInfo {
            title,
            time_pos,
            duration,
            volume,
        };

        if paused {
            Ok(PlayerState::Paused(info))
        } else {
            Ok(PlayerState::Playing(info))
        }
    }

    pub fn stop(&mut self) {
        if let Some(ref mut stream) = self.stream {
            let _ = send_command_on_stream(stream, &[json!("quit")]);
        } else if self.socket_path.exists() {
            if let Ok(mut stream) = UnixStream::connect(&self.socket_path) {
                let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                let _ = send_command_on_stream(&mut stream, &[json!("quit")]);
            }
        }

        if let Some(ref mut child) = self.process {
            // Give mpv up to 500ms to exit gracefully
            for _ in 0..5 {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(100)),
                    Err(_) => break,
                }
            }
            // Force kill if still running
            let _ = child.kill();
            let _ = child.wait(); // reap zombie
        }

        self.cleanup();
    }

    pub fn detach(&mut self) {
        self.process = None;
        self.stream = None;
        self.owns_process = false;
    }

    fn cleanup(&mut self) {
        self.stream = None;
        self.process = None;
        self.owns_process = false;
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_socket_path_is_configured() {
        let socket_path = PathBuf::from("/tmp/test-mpv.sock");
        let player = MpvPlayer::new(socket_path.clone());
        assert_eq!(player.socket_path(), socket_path.as_path());
    }

    #[test]
    fn test_new_player_is_stopped() {
        let mut player = MpvPlayer::new(PathBuf::from("/tmp/test-mpv-stopped.sock"));
        match player.poll_state().unwrap() {
            PlayerState::Stopped => {}
            _ => panic!("new player should be stopped"),
        }
    }

    #[test]
    fn test_build_launch_args_enable_long_video_buffering() {
        let config = Config::default();
        let args = build_launch_args(
            Path::new("/tmp/test.sock"),
            "https://www.youtube.com/watch?v=abc123",
            PlayMode::Video,
            PlaybackQuality::P1080,
            &config,
            None,
            Path::new("/tmp/mpv.log"),
            None,
        );

        assert!(args.iter().any(|arg| arg == "--hwdec=auto-safe"));
        assert!(args.iter().any(|arg| arg == "--cache=yes"));
        assert!(args.iter().any(|arg| arg == "--cache-pause=yes"));
        assert!(args
            .iter()
            .any(|arg| arg == &format!("--cache-secs={}", config.mpv_cache_secs)));
        assert!(args
            .iter()
            .any(|arg| arg == &format!("--cache-pause-wait={}", config.mpv_cache_pause_wait)));
        assert!(args
            .iter()
            .any(|arg| arg == &format!("--demuxer-max-bytes={}", config.mpv_demuxer_max_bytes)));
        assert!(args.iter().any(|arg| {
            arg == &format!(
                "--demuxer-max-back-bytes={}",
                config.mpv_demuxer_max_back_bytes
            )
        }));
        assert!(args.iter().any(|arg| arg == "--ontop"));
        assert!(args
            .iter()
            .any(|arg| arg
                == "--ytdl-format=bestvideo[vcodec!*=av01][height<=1080]+bestaudio/best[height<=1080]/best"));
    }

    #[test]
    fn test_build_launch_args_use_no_video_for_audio_mode() {
        let config = Config::default();
        let args = build_launch_args(
            Path::new("/tmp/test.sock"),
            "https://www.youtube.com/watch?v=abc123",
            PlayMode::AudioOnly,
            PlaybackQuality::P720,
            &config,
            None,
            Path::new("/tmp/mpv.log"),
            None,
        );

        assert!(args.iter().any(|arg| arg == "--no-video"));
        assert!(!args.iter().any(|arg| arg.starts_with("--geometry=")));
        assert!(args
            .iter()
            .any(|arg| arg
                == "--ytdl-format=bestvideo[vcodec!*=av01][height<=720]+bestaudio/best[height<=720]/best"));
    }

    #[test]
    fn test_build_launch_args_uses_geometry_override_when_present() {
        let config = Config::default();
        let args = build_launch_args(
            Path::new("/tmp/test.sock"),
            "https://www.youtube.com/watch?v=abc123",
            PlayMode::Video,
            PlaybackQuality::P1080,
            &config,
            Some("50%x50%+10+20"),
            Path::new("/tmp/mpv.log"),
            None,
        );

        assert!(args.iter().any(|arg| arg == "--geometry=50%x50%+10+20"));
    }
}
