-- Create bing_images table
CREATE TABLE bing_images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    copyright TEXT,
    copyright_link TEXT,
    market_code TEXT NOT NULL,
    fetched_at INTEGER NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_bing_images_url ON bing_images(url);
CREATE INDEX idx_bing_images_status ON bing_images(status);
CREATE INDEX idx_bing_images_market_code ON bing_images(market_code);
CREATE INDEX idx_bing_images_market_status ON bing_images(market_code, status);

-- Create market_codes table
CREATE TABLE market_codes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL UNIQUE,
    last_used_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_market_codes_code ON market_codes(code);

-- Create config_kv table
CREATE TABLE config_kv (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    value TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_config_kv_key ON config_kv(key);
