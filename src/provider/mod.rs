pub mod rustypipe_provider;

use crate::app::{SearchDate, SearchItemType, SearchLength, SearchSort};
use crate::models::*;
use anyhow::Result;
use async_trait::async_trait;

/// Options for filtered search.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub sort: Option<SearchSort>,
    pub date: Option<SearchDate>,
    pub item_type: Option<SearchItemType>,
    pub length: Option<SearchLength>,
}

/// Abstraction over YouTube data fetching, allowing different backends.
#[async_trait]
#[allow(dead_code)]
pub trait ContentProvider: Send + Sync {
    /// Search YouTube for videos, channels, and playlists.
    async fn search(&self, query: &str, continuation: Option<&str>) -> Result<FeedPage<FeedItem>>;

    /// Search YouTube with filters.
    async fn search_filtered(
        &self,
        query: &str,
        options: &SearchOptions,
    ) -> Result<FeedPage<FeedItem>>;

    /// Get detailed information about a single video.
    async fn video_detail(&self, id: &str) -> Result<VideoDetail>;

    /// Get channel metadata.
    async fn channel(&self, id: &str) -> Result<ChannelDetail>;

    /// Get playlist metadata and videos.
    async fn playlist(&self, id: &str) -> Result<PlaylistDetail>;
}
