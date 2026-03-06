use crate::models::{HistoryEntry, VideoItem};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::time::Duration;

use super::Database;

#[allow(dead_code)]
impl Database {
    pub fn add_to_history(
        &self,
        video_id: &str,
        title: &str,
        channel: &str,
        channel_id: &str,
        thumbnail_url: &str,
        duration: Option<Duration>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO watch_history (video_id, title, channel, channel_id, thumbnail_url, duration_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                video_id,
                title,
                channel,
                channel_id,
                thumbnail_url,
                duration.map(|d| d.as_secs() as i64),
            ],
        )?;
        Ok(())
    }

    pub fn get_history(&self, limit: u32, offset: u32) -> Result<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT video_id, title, channel, channel_id, thumbnail_url, duration_secs, watched_at
             FROM watch_history ORDER BY watched_at DESC LIMIT ?1 OFFSET ?2",
        )?;

        let entries = stmt
            .query_map(params![limit, offset], |row| {
                let duration_secs: Option<i64> = row.get(5)?;
                let watched_at_str: String = row.get(6)?;
                Ok(HistoryEntry {
                    video: VideoItem {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        channel: row.get(2)?,
                        channel_id: row.get(3)?,
                        thumbnail_url: row.get(4)?,
                        duration: duration_secs.map(|s| Duration::from_secs(s as u64)),
                        published: None,
                        view_count: None,
                    },
                    watched_at: DateTime::parse_from_rfc3339(&watched_at_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn clear_history(&self) -> Result<()> {
        self.conn.execute("DELETE FROM watch_history", [])?;
        Ok(())
    }
}
