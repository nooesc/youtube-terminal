use crate::models::{ItemType, ThumbnailKey};
use anyhow::Result;
use rusqlite::params;
use std::path::{Path, PathBuf};

use super::Database;

impl Database {
    pub fn get_cached_metadata(&self, video_id: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT json_data FROM metadata_cache WHERE video_id = ?1
             AND datetime(cached_at) > datetime('now', '-24 hours')",
            params![video_id],
            |row| row.get(0),
        );

        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_cached_metadata(&self, video_id: &str, json_data: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata_cache (video_id, json_data, cached_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            params![video_id, json_data],
        )?;
        Ok(())
    }

    pub fn get_thumbnail_path(&self, key: &ThumbnailKey) -> Result<Option<PathBuf>> {
        let item_type_str = match key.item_type {
            ItemType::Video => "video",
            ItemType::Channel => "channel",
            ItemType::Playlist => "playlist",
        };

        let result = self.conn.query_row(
            "SELECT file_path FROM thumbnail_index WHERE item_type = ?1 AND item_id = ?2",
            params![item_type_str, key.item_id],
            |row| {
                let path: String = row.get(0)?;
                Ok(PathBuf::from(path))
            },
        );

        match result {
            Ok(path) => Ok(Some(path)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_thumbnail_path(&self, key: &ThumbnailKey, path: &Path) -> Result<()> {
        let item_type_str = match key.item_type {
            ItemType::Video => "video",
            ItemType::Channel => "channel",
            ItemType::Playlist => "playlist",
        };

        self.conn.execute(
            "INSERT OR REPLACE INTO thumbnail_index (item_type, item_id, file_path, cached_at)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            params![item_type_str, key.item_id, path.to_string_lossy().as_ref()],
        )?;
        Ok(())
    }

    pub fn cleanup_old_thumbnails(
        &self,
        max_age_days: u32,
        max_count: u32,
    ) -> Result<Vec<PathBuf>> {
        let age_modifier = format!("-{} days", max_age_days);

        // Get paths of thumbnails to delete (expired by age)
        let mut stmt = self.conn.prepare(
            "SELECT file_path FROM thumbnail_index
             WHERE datetime(cached_at) < datetime('now', ?1)
             ORDER BY cached_at ASC",
        )?;

        let old_paths: Vec<PathBuf> = stmt
            .query_map(params![age_modifier], |row| {
                let path: String = row.get(0)?;
                Ok(PathBuf::from(path))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // Delete old entries
        self.conn.execute(
            "DELETE FROM thumbnail_index WHERE datetime(cached_at) < datetime('now', ?1)",
            params![age_modifier],
        )?;

        // Also trim if over max count
        let count: u32 =
            self.conn
                .query_row("SELECT COUNT(*) FROM thumbnail_index", [], |row| row.get(0))?;

        let mut excess_paths = Vec::new();
        if count > max_count {
            let mut stmt = self
                .conn
                .prepare("SELECT file_path FROM thumbnail_index ORDER BY cached_at ASC LIMIT ?1")?;
            excess_paths = stmt
                .query_map(params![count - max_count], |row| {
                    let path: String = row.get(0)?;
                    Ok(PathBuf::from(path))
                })?
                .filter_map(|r| r.ok())
                .collect();

            self.conn.execute(
                "DELETE FROM thumbnail_index WHERE rowid IN (
                    SELECT rowid FROM thumbnail_index ORDER BY cached_at ASC LIMIT ?1
                )",
                params![count - max_count],
            )?;
        }

        let mut all_paths = old_paths;
        all_paths.extend(excess_paths);
        Ok(all_paths)
    }
}
