-- Rollback: remove crop_coords column
ALTER TABLE bing_images DROP COLUMN crop_coords;
