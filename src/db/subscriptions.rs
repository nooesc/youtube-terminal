use crate::models::ChannelItem;
use anyhow::Result;
use rusqlite::params;

use super::Database;

#[allow(dead_code)]
impl Database {
    pub fn subscribe(&self, channel: &ChannelItem) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO subscriptions (channel_id, channel_name, thumbnail_url, subscriber_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                channel.id,
                channel.name,
                channel.thumbnail_url,
                channel.subscriber_count.map(|c| c as i64),
            ],
        )?;
        Ok(())
    }

    pub fn unsubscribe(&self, channel_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM subscriptions WHERE channel_id = ?1",
            params![channel_id],
        )?;
        Ok(())
    }

    pub fn is_subscribed(&self, channel_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM subscriptions WHERE channel_id = ?1",
            params![channel_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn get_subscriptions(&self) -> Result<Vec<ChannelItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT channel_id, channel_name, thumbnail_url, subscriber_count
             FROM subscriptions ORDER BY channel_name COLLATE NOCASE",
        )?;

        let items = stmt
            .query_map([], |row| {
                let subscriber_count: Option<i64> = row.get(3)?;
                Ok(ChannelItem {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    thumbnail_url: row.get(2)?,
                    subscriber_count: subscriber_count.map(|c| c as u64),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    pub fn get_subscribed_channel_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT channel_id FROM subscriptions")?;

        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids)
    }
}
