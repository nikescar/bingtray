use diesel::prelude::*;

use crate::core::sqlite::schema::{metadata, market};

// https://github.com/diesel-rs/diesel/tree/master/examples/sqlite/relations
#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug, Clone)]
#[diesel(table_name = metadata)]
pub struct Metadata {
    pub id: i32,
    pub blacklisted: bool,
    pub title: String,
    pub author: String,
    pub description: String,
    pub copyright: String,
    pub copyright_link: String,
    pub thumbnail_url: String,
    pub full_url: String,
}


#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug, Clone)]
#[diesel(table_name = market)]
pub struct Market {
    pub id: i32,
    pub mkcode: String,
    pub lastvisit: String,
}

