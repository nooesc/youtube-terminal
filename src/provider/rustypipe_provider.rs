use super::{AuthCapabilities, ContentProvider};
use crate::models;
use anyhow::{Context, Result};
use async_trait::async_trait;
use rustypipe::client::RustyPipe;
use rustypipe::model::paginator::ContinuationEndpoint;
use rustypipe::model::richtext::ToPlaintext;
use rustypipe::model::{
    ChannelItem as RpChannelItem, PlaylistItem as RpPlaylistItem, Thumbnail,
    VideoItem as RpVideoItem, YouTubeItem,
};
use std::path::Path;
use std::time::Duration;

/// YouTube data provider backed by the RustyPipe library.
pub struct RustyPipeProvider {
    client: RustyPipe,
    authenticated: bool,
}

impl RustyPipeProvider {
    /// Create a new provider, storing RustyPipe cache in `storage_dir`.
    pub async fn new(storage_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(storage_dir)?;
        let client = RustyPipe::builder()
            .storage_dir(storage_dir)
            .build()
            .context("failed to create RustyPipe client")?;
        Ok(Self {
            client,
            authenticated: false,
        })
    }

    /// Import cookies from Netscape cookie-jar text format.
    pub async fn set_cookies(&self, cookie_content: &str) -> Result<()> {
        self.client
            .user_auth_set_cookie_txt(cookie_content)
            .await
            .context("failed to set cookies")?;
        Ok(())
    }

    /// Mark this provider as authenticated (or not).
    pub fn set_authenticated(&mut self, auth: bool) {
        self.authenticated = auth;
    }
}

// ---------------------------------------------------------------------------
// Mapping helpers: RustyPipe types -> our models
// ---------------------------------------------------------------------------

/// Pick the largest thumbnail URL from a slice.
fn best_thumbnail_url(thumbnails: &[Thumbnail]) -> String {
    thumbnails
        .iter()
        .max_by_key(|t| t.width * t.height)
        .map(|t| t.url.clone())
        .unwrap_or_default()
}

/// Convert a `time::OffsetDateTime` to `chrono::DateTime<Utc>`.
fn to_chrono(dt: time::OffsetDateTime) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(dt.unix_timestamp(), 0).unwrap_or_default()
}

fn map_video_item(rp: &RpVideoItem) -> models::VideoItem {
    models::VideoItem {
        id: rp.id.clone(),
        title: rp.name.clone(),
        channel: rp
            .channel
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_default(),
        channel_id: rp
            .channel
            .as_ref()
            .map(|c| c.id.clone())
            .unwrap_or_default(),
        view_count: rp.view_count,
        duration: rp.duration.map(|d| Duration::from_secs(d as u64)),
        published: rp.publish_date.map(to_chrono),
        thumbnail_url: best_thumbnail_url(&rp.thumbnail),
    }
}

fn map_channel_item(rp: &RpChannelItem) -> models::ChannelItem {
    models::ChannelItem {
        id: rp.id.clone(),
        name: rp.name.clone(),
        subscriber_count: rp.subscriber_count,
        thumbnail_url: best_thumbnail_url(&rp.avatar),
    }
}

fn map_playlist_item(rp: &RpPlaylistItem) -> models::PlaylistItem {
    models::PlaylistItem {
        id: rp.id.clone(),
        title: rp.name.clone(),
        channel: rp
            .channel
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_default(),
        video_count: rp.video_count.map(|n| n as u32),
        thumbnail_url: best_thumbnail_url(&rp.thumbnail),
    }
}

fn map_youtube_item(item: &YouTubeItem) -> models::FeedItem {
    match item {
        YouTubeItem::Video(v) => {
            if v.is_short {
                models::FeedItem::Short(map_video_item(v))
            } else {
                models::FeedItem::Video(map_video_item(v))
            }
        }
        YouTubeItem::Channel(c) => models::FeedItem::Channel(map_channel_item(c)),
        YouTubeItem::Playlist(p) => models::FeedItem::Playlist(map_playlist_item(p)),
    }
}

// ---------------------------------------------------------------------------
// ContentProvider implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ContentProvider for RustyPipeProvider {
    fn capabilities(&self) -> AuthCapabilities {
        AuthCapabilities {
            has_home_feed: self.authenticated,
            has_subscriptions: self.authenticated,
            has_history: self.authenticated,
        }
    }

    async fn search(
        &self,
        query: &str,
        continuation: Option<&str>,
    ) -> Result<models::FeedPage<models::FeedItem>> {
        if let Some(ctoken) = continuation {
            let page = self
                .client
                .query()
                .continuation::<YouTubeItem, _>(ctoken, ContinuationEndpoint::Search, None)
                .await
                .context("search continuation failed")?;
            Ok(models::FeedPage {
                items: page.items.iter().map(map_youtube_item).collect(),
                continuation: page.ctoken,
            })
        } else {
            let result = self
                .client
                .query()
                .search::<YouTubeItem, _>(query)
                .await
                .context("search failed")?;
            Ok(models::FeedPage {
                items: result.items.items.iter().map(map_youtube_item).collect(),
                continuation: result.items.ctoken,
            })
        }
    }

    async fn trending(&self) -> Result<models::FeedPage<models::VideoItem>> {
        let items = self
            .client
            .query()
            .trending()
            .await
            .context("trending failed")?;
        Ok(models::FeedPage {
            items: items.iter().map(map_video_item).collect(),
            continuation: None, // trending is not paginated
        })
    }

    async fn video_detail(&self, id: &str) -> Result<models::VideoDetail> {
        let details = self
            .client
            .query()
            .video_details(id)
            .await
            .context("video_details failed")?;

        Ok(models::VideoDetail {
            item: models::VideoItem {
                id: details.id.clone(),
                title: details.name.clone(),
                channel: details.channel.name.clone(),
                channel_id: details.channel.id.clone(),
                view_count: Some(details.view_count),
                duration: None, // VideoDetails doesn't carry duration
                published: details.publish_date.map(to_chrono),
                thumbnail_url: String::new(), // not needed for detail view
            },
            description: details.description.to_plaintext(),
            like_count: details.like_count.map(|n| n as u64),
            keywords: Vec::new(), // VideoDetails doesn't expose keywords
        })
    }

    async fn channel(&self, id: &str) -> Result<models::ChannelDetail> {
        // channel_videos returns Channel<Paginator<VideoItem>> which carries
        // all the metadata we need (name, avatar, description, counts).
        let ch = self
            .client
            .query()
            .channel_videos(id)
            .await
            .context("channel_videos failed")?;

        Ok(models::ChannelDetail {
            item: models::ChannelItem {
                id: ch.id.clone(),
                name: ch.name.clone(),
                subscriber_count: ch.subscriber_count,
                thumbnail_url: best_thumbnail_url(&ch.avatar),
            },
            description: ch.description.clone(),
            video_count: ch.video_count,
        })
    }

    async fn channel_videos(
        &self,
        id: &str,
        continuation: Option<&str>,
    ) -> Result<models::FeedPage<models::VideoItem>> {
        if let Some(ctoken) = continuation {
            let page = self
                .client
                .query()
                .continuation::<RpVideoItem, _>(ctoken, ContinuationEndpoint::Browse, None)
                .await
                .context("channel_videos continuation failed")?;
            Ok(models::FeedPage {
                items: page.items.iter().map(map_video_item).collect(),
                continuation: page.ctoken,
            })
        } else {
            let channel = self
                .client
                .query()
                .channel_videos(id)
                .await
                .context("channel_videos failed")?;
            Ok(models::FeedPage {
                items: channel.content.items.iter().map(map_video_item).collect(),
                continuation: channel.content.ctoken,
            })
        }
    }

    async fn home_feed(
        &self,
        _continuation: Option<&str>,
    ) -> Result<models::FeedPage<models::FeedItem>> {
        // RustyPipe doesn't expose a home/recommended feed API,
        // so we fall back to trending wrapped as FeedItem::Video.
        let page = self.trending().await?;
        Ok(models::FeedPage {
            items: page.items.into_iter().map(models::FeedItem::Video).collect(),
            continuation: None,
        })
    }

    async fn subscriptions(
        &self,
        _continuation: Option<&str>,
    ) -> Result<models::FeedPage<models::ChannelItem>> {
        // RustyPipeQuery::subscriptions() already calls .authenticated() internally
        let page = self
            .client
            .query()
            .subscriptions()
            .await
            .context("subscriptions failed")?;
        Ok(models::FeedPage {
            items: page.items.iter().map(map_channel_item).collect(),
            continuation: page.ctoken,
        })
    }

    async fn subscription_feed(
        &self,
        _continuation: Option<&str>,
    ) -> Result<models::FeedPage<models::VideoItem>> {
        // RustyPipeQuery::subscription_feed() already calls .authenticated() internally
        let page = self
            .client
            .query()
            .subscription_feed()
            .await
            .context("subscription_feed failed")?;
        Ok(models::FeedPage {
            items: page.items.iter().map(map_video_item).collect(),
            continuation: page.ctoken,
        })
    }
}
