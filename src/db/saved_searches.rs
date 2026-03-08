use crate::app::{SearchDate, SearchItemType, SearchLength, SearchSort};
use anyhow::Result;
use rusqlite::params;

use super::Database;

#[derive(Debug, Clone)]
pub struct SavedSearch {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub sort: SearchSort,
    pub date: SearchDate,
    pub item_type: SearchItemType,
    pub length: SearchLength,
    pub created_at: String,
    pub last_run_at: Option<String>,
}

fn parse_sort(s: &str) -> SearchSort {
    match s {
        "Date" => SearchSort::Date,
        "Views" => SearchSort::Views,
        "Rating" => SearchSort::Rating,
        _ => SearchSort::Relevance,
    }
}

fn parse_date(s: &str) -> SearchDate {
    match s {
        "Hour" => SearchDate::Hour,
        "Day" => SearchDate::Day,
        "Week" => SearchDate::Week,
        "Month" => SearchDate::Month,
        "Year" => SearchDate::Year,
        _ => SearchDate::Any,
    }
}

fn parse_item_type(s: &str) -> SearchItemType {
    match s {
        "Video" => SearchItemType::Video,
        "Channel" => SearchItemType::Channel,
        "Playlist" => SearchItemType::Playlist,
        _ => SearchItemType::All,
    }
}

fn parse_length(s: &str) -> SearchLength {
    match s {
        "Short" => SearchLength::Short,
        "Medium" => SearchLength::Medium,
        "Long" => SearchLength::Long,
        _ => SearchLength::Any,
    }
}

impl Database {
    pub fn save_search(
        &self,
        name: &str,
        query: &str,
        sort: SearchSort,
        date: SearchDate,
        item_type: SearchItemType,
        length: SearchLength,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO saved_searches (name, query, sort, date, item_type, length)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                name,
                query,
                sort.label(),
                date.label(),
                item_type.label(),
                length.label(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_saved_searches(&self) -> Result<Vec<SavedSearch>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, query, sort, date, item_type, length, created_at, last_run_at
             FROM saved_searches ORDER BY created_at DESC",
        )?;

        let items = stmt
            .query_map([], |row| {
                let sort_str: String = row.get(3)?;
                let date_str: String = row.get(4)?;
                let item_type_str: String = row.get(5)?;
                let length_str: String = row.get(6)?;
                Ok(SavedSearch {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    query: row.get(2)?,
                    sort: parse_sort(&sort_str),
                    date: parse_date(&date_str),
                    item_type: parse_item_type(&item_type_str),
                    length: parse_length(&length_str),
                    created_at: row.get(7)?,
                    last_run_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    pub fn delete_saved_search(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM saved_searches WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn rename_saved_search(&self, id: i64, new_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE saved_searches SET name = ?1 WHERE id = ?2",
            params![new_name, id],
        )?;
        Ok(())
    }

    pub fn update_last_run(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE saved_searches SET last_run_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}
