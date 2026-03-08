pub mod cache;
pub mod history;
pub mod saved_searches;
pub mod subscriptions;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Database { conn };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS watch_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                video_id TEXT NOT NULL,
                title TEXT NOT NULL,
                channel TEXT NOT NULL,
                channel_id TEXT NOT NULL,
                thumbnail_url TEXT NOT NULL DEFAULT '',
                duration_secs INTEGER,
                watched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE INDEX IF NOT EXISTS idx_history_watched_at ON watch_history(watched_at DESC);
            CREATE INDEX IF NOT EXISTS idx_history_video_id ON watch_history(video_id);

            CREATE TABLE IF NOT EXISTS metadata_cache (
                video_id TEXT PRIMARY KEY,
                json_data TEXT NOT NULL,
                cached_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE TABLE IF NOT EXISTS thumbnail_index (
                item_type TEXT NOT NULL,
                item_id TEXT NOT NULL,
                file_path TEXT NOT NULL,
                cached_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                PRIMARY KEY (item_type, item_id)
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS subscriptions (
                channel_id TEXT PRIMARY KEY,
                channel_name TEXT NOT NULL,
                thumbnail_url TEXT NOT NULL DEFAULT '',
                subscriber_count INTEGER,
                subscribed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );

            CREATE TABLE IF NOT EXISTS saved_searches (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                query TEXT NOT NULL,
                sort TEXT NOT NULL DEFAULT 'Relevance',
                date TEXT NOT NULL DEFAULT 'Any',
                item_type TEXT NOT NULL DEFAULT 'All',
                length TEXT NOT NULL DEFAULT 'Any',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                last_run_at TEXT
            );",
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{SearchDate, SearchItemType, SearchLength, SearchSort};
    use crate::models::{ChannelItem, ItemType, ThumbnailKey};
    use std::path::PathBuf;

    fn test_db() -> Database {
        Database::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_history_insert_and_retrieve() {
        let db = test_db();
        db.add_to_history(
            "abc123",
            "Test Video",
            "Test Channel",
            "ch1",
            "http://thumb.jpg",
            Some(std::time::Duration::from_secs(120)),
        )
        .unwrap();
        let history = db.get_history(10, 0).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].video.id, "abc123");
        assert_eq!(history[0].video.title, "Test Video");
    }

    #[test]
    fn test_history_clear() {
        let db = test_db();
        db.add_to_history("v1", "Vid 1", "Ch", "c1", "", None)
            .unwrap();
        db.add_to_history("v2", "Vid 2", "Ch", "c1", "", None)
            .unwrap();
        assert_eq!(db.get_history(10, 0).unwrap().len(), 2);
        db.clear_history().unwrap();
        assert_eq!(db.get_history(10, 0).unwrap().len(), 0);
    }

    #[test]
    fn test_metadata_cache() {
        let db = test_db();
        assert!(db.get_cached_metadata("vid1").unwrap().is_none());
        db.set_cached_metadata("vid1", r#"{"title":"test"}"#)
            .unwrap();
        let cached = db.get_cached_metadata("vid1").unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), r#"{"title":"test"}"#);
    }

    #[test]
    fn test_thumbnail_index() {
        let db = test_db();
        let key = ThumbnailKey {
            item_type: ItemType::Video,
            item_id: "vid1".into(),
        };
        assert!(db.get_thumbnail_path(&key).unwrap().is_none());
        db.set_thumbnail_path(&key, &PathBuf::from("/tmp/thumb.jpg"))
            .unwrap();
        let path = db.get_thumbnail_path(&key).unwrap().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/thumb.jpg"));
    }

    #[test]
    fn test_subscribe_and_list() {
        let db = test_db();
        let channel = ChannelItem {
            id: "UC123".into(),
            name: "Test Channel".into(),
            subscriber_count: Some(1000),
            thumbnail_url: "http://thumb.jpg".into(),
        };
        db.subscribe(&channel).unwrap();
        assert!(db.is_subscribed("UC123").unwrap());
        let subs = db.get_subscriptions().unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].id, "UC123");
        assert_eq!(subs[0].name, "Test Channel");
        assert_eq!(subs[0].subscriber_count, Some(1000));
        assert_eq!(subs[0].thumbnail_url, "http://thumb.jpg");
    }

    #[test]
    fn test_unsubscribe() {
        let db = test_db();
        let channel = ChannelItem {
            id: "UC456".into(),
            name: "Another Channel".into(),
            subscriber_count: None,
            thumbnail_url: "".into(),
        };
        db.subscribe(&channel).unwrap();
        assert!(db.is_subscribed("UC456").unwrap());
        db.unsubscribe("UC456").unwrap();
        assert!(!db.is_subscribed("UC456").unwrap());
        assert_eq!(db.get_subscriptions().unwrap().len(), 0);
    }

    #[test]
    fn test_save_and_list_searches() {
        let db = test_db();
        let id = db
            .save_search(
                "Rust tutorials",
                "rust programming",
                SearchSort::Views,
                SearchDate::Month,
                SearchItemType::Video,
                SearchLength::Long,
            )
            .unwrap();
        assert!(id > 0);

        let searches = db.get_saved_searches().unwrap();
        assert_eq!(searches.len(), 1);
        assert_eq!(searches[0].id, id);
        assert_eq!(searches[0].name, "Rust tutorials");
        assert_eq!(searches[0].query, "rust programming");
        assert_eq!(searches[0].sort, SearchSort::Views);
        assert_eq!(searches[0].date, SearchDate::Month);
        assert_eq!(searches[0].item_type, SearchItemType::Video);
        assert_eq!(searches[0].length, SearchLength::Long);
        assert!(!searches[0].created_at.is_empty());
        assert!(searches[0].last_run_at.is_none());
    }

    #[test]
    fn test_delete_saved_search() {
        let db = test_db();
        let id = db
            .save_search(
                "Delete me",
                "query",
                SearchSort::Relevance,
                SearchDate::Any,
                SearchItemType::All,
                SearchLength::Any,
            )
            .unwrap();
        assert_eq!(db.get_saved_searches().unwrap().len(), 1);
        db.delete_saved_search(id).unwrap();
        assert_eq!(db.get_saved_searches().unwrap().len(), 0);
    }

    #[test]
    fn test_rename_saved_search() {
        let db = test_db();
        let id = db
            .save_search(
                "Old name",
                "query",
                SearchSort::Relevance,
                SearchDate::Any,
                SearchItemType::All,
                SearchLength::Any,
            )
            .unwrap();
        db.rename_saved_search(id, "New name").unwrap();
        let searches = db.get_saved_searches().unwrap();
        assert_eq!(searches[0].name, "New name");
    }

    #[test]
    fn test_update_last_run() {
        let db = test_db();
        let id = db
            .save_search(
                "Test search",
                "query",
                SearchSort::Relevance,
                SearchDate::Any,
                SearchItemType::All,
                SearchLength::Any,
            )
            .unwrap();

        let searches = db.get_saved_searches().unwrap();
        assert!(searches[0].last_run_at.is_none());

        db.update_last_run(id).unwrap();

        let searches = db.get_saved_searches().unwrap();
        assert!(searches[0].last_run_at.is_some());
    }
}
