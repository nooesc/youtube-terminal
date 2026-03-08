use crate::models::{ItemType, ThumbnailKey};
use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView};
use ratatui::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Download a thumbnail from URL and save to cache directory.
pub async fn download_thumbnail(
    url: &str,
    key: &ThumbnailKey,
    cache_dir: &Path,
) -> Result<PathBuf> {
    std::fs::create_dir_all(cache_dir)?;

    let filename = format!(
        "{}_{}.jpg",
        match key.item_type {
            ItemType::Video => "video",
            ItemType::Channel => "channel",
            ItemType::Playlist => "playlist",
        },
        key.item_id
    );
    let path = cache_dir.join(&filename);

    let bytes = reqwest::get(url)
        .await
        .context("failed to download thumbnail")?
        .bytes()
        .await
        .context("failed to read thumbnail bytes")?;

    tokio::fs::write(&path, &bytes)
        .await
        .context("failed to write thumbnail")?;

    Ok(path)
}

/// Cache of loaded and resized images ready for rendering.
pub struct ThumbnailCache {
    cache: HashMap<ThumbnailKey, DynamicImage>,
    detail_cache: HashMap<ThumbnailKey, DynamicImage>,
    avatar_cache: HashMap<ThumbnailKey, DynamicImage>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            detail_cache: HashMap::new(),
            avatar_cache: HashMap::new(),
        }
    }

    /// Load a thumbnail from disk and resize it for a given cell area.
    ///
    /// `width` is the number of terminal columns, `height` is the number of terminal rows.
    /// Each terminal row renders two pixel rows via half-block characters, so the image
    /// is resized to (width, height * 2).
    /// Uses `load_from_memory` instead of `image::open` because YouTube returns
    /// WebP content even when the URL ends in `.jpg` — `image::open` guesses
    /// format from the file extension and fails on these files.
    pub fn load(&mut self, key: &ThumbnailKey, path: &Path, width: u32, height: u32) -> Result<()> {
        let data = std::fs::read(path).context("failed to read thumbnail file")?;
        let img = image::load_from_memory(&data).context("failed to decode thumbnail")?;
        let resized = img.resize_exact(width, height * 2, image::imageops::FilterType::Triangle);
        self.cache.insert(key.clone(), resized);
        Ok(())
    }

    /// Load a thumbnail at a larger size for the detail view.
    pub fn load_detail(&mut self, key: &ThumbnailKey, path: &Path, width: u32, height: u32) -> Result<()> {
        let data = std::fs::read(path).context("failed to read thumbnail file")?;
        let img = image::load_from_memory(&data).context("failed to decode thumbnail")?;
        let resized = img.resize_exact(width, height * 2, image::imageops::FilterType::Triangle);
        self.detail_cache.insert(key.clone(), resized);
        Ok(())
    }

    /// Get a cached thumbnail image.
    pub fn get(&self, key: &ThumbnailKey) -> Option<&DynamicImage> {
        self.cache.get(key)
    }

    /// Get a detail-sized cached thumbnail image.
    pub fn get_detail(&self, key: &ThumbnailKey) -> Option<&DynamicImage> {
        self.detail_cache.get(key)
    }

    /// Load a thumbnail as a channel avatar (square aspect).
    pub fn load_avatar(&mut self, key: &ThumbnailKey, path: &Path, size: u32) -> Result<()> {
        let data = std::fs::read(path).context("failed to read avatar file")?;
        let img = image::load_from_memory(&data).context("failed to decode avatar")?;
        let resized = img.resize_exact(size, size * 2, image::imageops::FilterType::Triangle);
        self.avatar_cache.insert(key.clone(), resized);
        Ok(())
    }

    /// Get a cached avatar image.
    pub fn get_avatar(&self, key: &ThumbnailKey) -> Option<&DynamicImage> {
        self.avatar_cache.get(key)
    }

    /// Render a thumbnail using half-block characters (U+2580 "UPPER HALF BLOCK")
    /// into a ratatui Buffer.
    ///
    /// The upper half-block char uses fg for the top pixel row and bg for the bottom
    /// pixel row, doubling effective vertical resolution.
    pub fn render_halfblock(img: &DynamicImage, area: Rect, buf: &mut Buffer) {
        let (img_w, img_h) = img.dimensions();

        for y in 0..area.height {
            for x in 0..area.width {
                let px = x as u32;
                let py_top = (y as u32) * 2;
                let py_bot = py_top + 1;

                if px >= img_w || py_top >= img_h {
                    continue;
                }

                let top_pixel = img.get_pixel(px, py_top);
                let fg = Color::Rgb(top_pixel[0], top_pixel[1], top_pixel[2]);

                let bg = if py_bot < img_h {
                    let bot_pixel = img.get_pixel(px, py_bot);
                    Color::Rgb(bot_pixel[0], bot_pixel[1], bot_pixel[2])
                } else {
                    Color::Black
                };

                if let Some(cell) = buf.cell_mut(Position::new(area.x + x, area.y + y)) {
                    cell.set_char('\u{2580}').set_fg(fg).set_bg(bg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumbnail_cache_new() {
        let cache = ThumbnailCache::new();
        let key = ThumbnailKey {
            item_type: ItemType::Video,
            item_id: "test123".to_string(),
        };
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_render_halfblock_empty_image() {
        // Create a tiny 2x2 image
        let img = DynamicImage::new_rgb8(2, 2);
        let area = Rect::new(0, 0, 2, 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        ThumbnailCache::render_halfblock(&img, area, &mut buf);

        // Should have written half-block chars in the area
        let cell = &buf[Position::new(0, 0)];
        assert_eq!(cell.symbol(), "\u{2580}");
    }
}
