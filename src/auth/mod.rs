pub mod cookies;

use crate::config::Config;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum AuthState {
    NoAuth,
    Authenticated { cookie_path: PathBuf },
}

#[allow(dead_code)]
impl AuthState {
    pub fn load(config: &Config) -> Self {
        let path = config.cookie_path();
        if path.exists()
            && std::fs::metadata(&path)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
            AuthState::Authenticated { cookie_path: path }
        } else {
            AuthState::NoAuth
        }
    }

    pub fn cookie_path(&self) -> Option<&Path> {
        match self {
            AuthState::NoAuth => None,
            AuthState::Authenticated { cookie_path } => Some(cookie_path),
        }
    }

    pub fn is_authenticated(&self) -> bool {
        matches!(self, AuthState::Authenticated { .. })
    }
}
