-- Add cached_at column to track when image bytes were downloaded to local cache
ALTER TABLE bing_images ADD COLUMN cached_at INTEGER;
