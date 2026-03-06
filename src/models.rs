use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A page of results from YouTube with an optional continuation token
#[derive(Debug, Clone)]
pub struct FeedPage<T> {
    pub items: Vec<T>,
    pub continuation: Option<String>,
}

/// Items that can appear in feeds — not just videos
#[derive(Debug, Clone)]
pub enum FeedItem {
    Video(VideoItem),
    Channel(ChannelItem),
    Playlist(PlaylistItem),
    Short(VideoItem),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDetail {
    pub item: VideoItem,
    pub description: String,
    pub like_count: Option<u64>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelItem {
    pub id: String,
    pub name: String,
    pub subscriber_count: Option<u64>,
    pub thumbnail_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub video_count: Option<u32>,
    pub thumbnail_url: String,
}

/// History items carry extra metadata about when the video was watched
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub video: VideoItem,
    pub watched_at: DateTime<Utc>,
}

/// Typed cache key for thumbnails
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ThumbnailKey {
    pub item_type: ItemType,
    pub item_id: String,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ItemType {
    Video,
    Channel,
    Playlist,
}

/// Search filters (placeholder for now)
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SearchFilters {}

/// Channel detail view
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChannelDetail {
    pub item: ChannelItem,
    pub description: String,
    pub video_count: Option<u64>,
}

#[allow(dead_code)]
impl FeedItem {
    pub fn thumbnail_key(&self) -> ThumbnailKey {
        match self {
            FeedItem::Video(v) | FeedItem::Short(v) => ThumbnailKey {
                item_type: ItemType::Video,
                item_id: v.id.clone(),
            },
            FeedItem::Channel(c) => ThumbnailKey {
                item_type: ItemType::Channel,
                item_id: c.id.clone(),
            },
            FeedItem::Playlist(p) => ThumbnailKey {
                item_type: ItemType::Playlist,
                item_id: p.id.clone(),
            },
        }
    }

    pub fn thumbnail_url(&self) -> &str {
        match self {
            FeedItem::Video(v) | FeedItem::Short(v) => &v.thumbnail_url,
            FeedItem::Channel(c) => &c.thumbnail_url,
            FeedItem::Playlist(p) => &p.thumbnail_url,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            FeedItem::Video(v) | FeedItem::Short(v) => &v.title,
            FeedItem::Channel(c) => &c.name,
            FeedItem::Playlist(p) => &p.title,
        }
    }
}
