// Simple schema definition for posts
// This is a placeholder until we implement proper SQLite integration

pub struct PostsTable;

impl PostsTable {
    pub const TABLE_NAME: &'static str = "posts";
    pub const ID_COLUMN: &'static str = "id";
    pub const TITLE_COLUMN: &'static str = "title";
    pub const BODY_COLUMN: &'static str = "body";
    pub const PUBLISHED_COLUMN: &'static str = "published";
}