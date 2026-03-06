pub mod rustypipe_provider;

use crate::models::*;
use anyhow::Result;
use async_trait::async_trait;

/// Describes what authenticated features are available.
#[allow(dead_code)]
pub struct AuthCapabilities {
    pub has_home_feed: bool,
    pub has_subscriptions: bool,
    pub has_history: bool,
}

/// Abstraction over YouTube data fetching, allowing different backends.
#[async_trait]
#[allow(dead_code)]
pub trait ContentProvider: Send + Sync {
    /// Returns which authenticated features are currently available.
    fn capabilities(&self) -> AuthCapabilities;

    // -- Unauthenticated endpoints --

    /// Search YouTube for videos, channels, and playlists.
    async fn search(&self, query: &str, continuation: Option<&str>) -> Result<FeedPage<FeedItem>>;

    /// Get trending videos (not paginated).
    async fn trending(&self) -> Result<FeedPage<VideoItem>>;

    /// Get detailed information about a single video.
    async fn video_detail(&self, id: &str) -> Result<VideoDetail>;

    /// Get channel metadata.
    async fn channel(&self, id: &str) -> Result<ChannelDetail>;

    /// Get a channel's uploaded videos (paginated).
    async fn channel_videos(
        &self,
        id: &str,
        continuation: Option<&str>,
    ) -> Result<FeedPage<VideoItem>>;

    // -- Authenticated endpoints (require cookies) --

    /// Get the user's home/recommended feed.
    async fn home_feed(&self, continuation: Option<&str>) -> Result<FeedPage<FeedItem>>;

    /// Get the user's subscribed channels.
    async fn subscriptions(&self, continuation: Option<&str>) -> Result<FeedPage<ChannelItem>>;

    /// Get the user's subscription video feed.
    async fn subscription_feed(&self, continuation: Option<&str>) -> Result<FeedPage<VideoItem>>;
}
