use diesel::prelude::*;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = crate::schema::bing_images)]
pub struct BingImage {
    pub id: i32,
    pub url: String,
    pub title: String,
    pub copyright: Option<String>,
    pub copyright_link: Option<String>,
    pub market_code: String,
    pub fetched_at: i32,
    pub status: String,
    pub created_at: i32,
    pub updated_at: i32,
    pub cached_at: Option<i32>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::bing_images)]
pub struct NewBingImage<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub copyright: Option<&'a str>,
    pub copyright_link: Option<&'a str>,
    pub market_code: &'a str,
    pub fetched_at: i32,
    pub status: &'a str,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = crate::schema::market_codes)]
pub struct MarketCode {
    pub id: i32,
    pub code: String,
    pub last_used_at: i32,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::market_codes)]
pub struct NewMarketCode<'a> {
    pub code: &'a str,
    pub last_used_at: i32,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = crate::schema::config_kv)]
pub struct ConfigKv {
    pub id: i32,
    pub key: String,
    pub value: String,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = crate::schema::config_kv)]
pub struct NewConfigKv<'a> {
    pub key: &'a str,
    pub value: &'a str,
    pub created_at: i32,
    pub updated_at: i32,
}

/// Image status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageStatus {
    Unprocessed,
    KeepFavorite,
    Blacklisted,
}

impl ImageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageStatus::Unprocessed => "unprocessed",
            ImageStatus::KeepFavorite => "keepfavorite",
            ImageStatus::Blacklisted => "blacklisted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "unprocessed" | "cached" => Some(ImageStatus::Unprocessed),
            "keepfavorite" => Some(ImageStatus::KeepFavorite),
            "blacklisted" => Some(ImageStatus::Blacklisted),
            _ => None,
        }
    }
}
