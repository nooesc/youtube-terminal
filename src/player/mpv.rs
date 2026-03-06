use super::{PlayMode, PlayerInfo, PlayerState};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub struct MpvPlayer {
    socket_path: PathBuf,
    process: Option<Child>,
    stream: Option<UnixStream>,
}

#[allow(dead_code)]
impl MpvPlayer {
    pub fn new() -> Self {
        let pid = std::process::id();
        let socket_path = PathBuf::from(format!("/tmp/yt-term-{}.sock", pid));
        // Clean up stale socket from previous crash
        let _ = std::fs::remove_file(&socket_path);
        Self {
            socket_path,
            process: None,
            stream: None,
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn play(
        &mut self,
        url: &str,
        mode: PlayMode,
        geometry: &str,
        ontop: bool,
        cookie_path: Option<&Path>,
    ) -> Result<()> {
        // Kill existing process if any
        self.stop();

        let mut cmd = Command::new("mpv");
        cmd.arg(format!("--input-ipc-server={}", self.socket_path.display()));

        match mode {
            PlayMode::Video => {
                cmd.arg(format!("--geometry={}", geometry));
                if ontop {
                    cmd.arg("--ontop");
                }
            }
            PlayMode::AudioOnly => {
                cmd.arg("--no-video");
            }
        }

        if let Some(cookie) = cookie_path {
            if cookie.exists() {
                cmd.arg(format!("--ytdl-raw-options=cookies={}", cookie.display()));
            }
        }

        cmd.arg(url);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd
            .spawn()
            .context("failed to spawn mpv — is it installed?")?;
        self.process = Some(child);

        // Wait for mpv to create the socket and connect
        self.connect_with_retry()?;

        Ok(())
    }

    fn connect_with_retry(&mut self) -> Result<()> {
        for i in 0..20 {
            if self.socket_path.exists() {
                match UnixStream::connect(&self.socket_path) {
                    Ok(stream) => {
                        stream.set_read_timeout(Some(std::time::Duration::from_millis(500)))?;
                        self.stream = Some(stream);
                        return Ok(());
                    }
                    Err(_) if i < 19 => {}
                    Err(e) => return Err(e.into()),
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        bail!("timed out waiting for mpv socket")
    }

    pub fn send_command(&mut self, cmd: &[Value]) -> Result<Option<Value>> {
        let stream = self.stream.as_mut().context("mpv not connected")?;

        let msg = json!({"command": cmd});
        let mut line = serde_json::to_string(&msg)?;
        line.push('\n');
        stream.write_all(line.as_bytes())?;
        stream.flush()?;

        // Read response
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
        self.send_command(&[json!("seek"), json!(seconds), json!("relative")])?;
        Ok(())
    }

    pub fn set_volume(&mut self, vol: f64) -> Result<()> {
        self.set_property("volume", json!(vol))
    }

    pub fn poll_state(&mut self) -> Result<PlayerState> {
        if self.stream.is_none() {
            return Ok(PlayerState::Stopped);
        }

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
            let msg = json!({"command": ["quit"]});
            let mut line = serde_json::to_string(&msg).unwrap_or_default();
            line.push('\n');
            let _ = stream.write_all(line.as_bytes());
            let _ = stream.flush();
        }

        if let Some(ref mut child) = self.process {
            let _ = child.wait(); // reap zombie
        }

        self.cleanup();
    }

    fn cleanup(&mut self) {
        self.stream = None;
        self.process = None;
        let _ = std::fs::remove_file(&self.socket_path);
    }

    pub fn is_running(&self) -> bool {
        self.stream.is_some()
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_path_contains_pid() {
        let player = MpvPlayer::new();
        let pid = std::process::id();
        assert!(player
            .socket_path()
            .to_string_lossy()
            .contains(&pid.to_string()));
    }

    #[test]
    fn test_new_player_is_stopped() {
        let mut player = MpvPlayer::new();
        match player.poll_state().unwrap() {
            PlayerState::Stopped => {}
            _ => panic!("new player should be stopped"),
        }
    }
}
