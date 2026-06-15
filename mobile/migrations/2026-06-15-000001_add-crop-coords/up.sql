-- Add crop_coords column to bing_images table
-- Stores JSON: {"x":0.1,"y":0.2,"width":0.6,"height":0.8}
-- Values are normalized floats (0.0-1.0) relative to image dimensions
ALTER TABLE bing_images ADD COLUMN crop_coords TEXT;
