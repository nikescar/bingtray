use diesel::prelude::*;
use log::info;
use std::error::Error;

pub mod model;
#[rustfmt::skip]
pub mod schema;

use self::model::*;
use self::schema::*;

type DbResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct Sqlite {
    connection: SqliteConnection,

}

impl Sqlite {
    pub fn new(db_path: &str) -> Result<Self, Box<dyn Error>> {
        let mut connection = Self::establish_connection(db_path)?;

        info!("Using SQLite database at: {:?}", db_path);

        // create metadata and market tables if not exists. create with diesel
        diesel::sql_query(
            "CREATE TABLE IF NOT EXISTS metadata (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                blacklisted BOOLEAN NOT NULL,
                title TEXT NOT NULL,
                author TEXT NOT NULL,
                description TEXT NOT NULL,
                copyright TEXT NOT NULL,
                copyright_link TEXT NOT NULL,
                thumbnail_url TEXT NOT NULL,
                full_url TEXT NOT NULL
            )",
        )
        .execute(&mut connection)?;

        diesel::sql_query(
            "CREATE TABLE IF NOT EXISTS market (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mkcode TEXT NOT NULL,
                lastvisit LONG NOT NULL
            )",
        )
        .execute(&mut connection)?;

        Ok(Sqlite { connection })
    }

    fn establish_connection(path: &str) -> Result<SqliteConnection, Box<dyn Error>> {
        // echo DATABASE_URL=/path/to/your/sqlite/database.db > .env
        let connection = SqliteConnection::establish(path)
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        Ok(connection)
    }

    // new metadata entry, title should be unique, check before insert
    pub fn new_metadata_entry(
        &mut self,
        blacklisted: bool,
        title: &str,
        author: &str,
        description: &str,
        copyright: &str,
        copyright_link: &str,
        thumbnail_url: &str,
        full_url: &str,
    ) -> DbResult<Metadata> {
        // Check if title already exists
        let existing = metadata::table
            .filter(metadata::title.eq(title))
            .first::<Metadata>(&mut self.connection)
            .optional()?;
        
        if existing.is_some() {
            return Err("Title already exists".into());
        }

        diesel::insert_into(metadata::table)
            .values((
            metadata::blacklisted.eq(blacklisted),
            metadata::title.eq(title),
            metadata::author.eq(author),
            metadata::description.eq(description),
            metadata::copyright.eq(copyright),
            metadata::copyright_link.eq(copyright_link),
            metadata::thumbnail_url.eq(thumbnail_url),
            metadata::full_url.eq(full_url),
            ))
            .execute(&mut self.connection)?;

        // Get the last inserted metadata
        let metadata = metadata::table
            .order(metadata::id.desc())
            .first(&mut self.connection)?;
        Ok(metadata)
    }

    // new market entry
    pub fn new_market_entry(&mut self, mkcode: &str, lastvisit: i64) -> DbResult<Market> {
        diesel::insert_into(market::table)
            .values((
                market::mkcode.eq(mkcode),
                market::lastvisit.eq(lastvisit),
            ))
            .execute(&mut self.connection)?;

        // Get the last inserted market
        let market = market::table
            .order(market::id.desc())
            .first(&mut self.connection)?;
        Ok(market)
    }

    // set blacklisted status by title
    pub fn set_blacklisted_status(&mut self, title: &str, blacklisted: bool) -> DbResult<usize> {
        let rows_updated = diesel::update(metadata::table.filter(metadata::title.eq(title)))
            .set(metadata::blacklisted.eq(blacklisted))
            .execute(&mut self.connection)?;
        Ok(rows_updated)
    }

    // get all metadata entries
    pub fn get_all_metadata(&mut self) -> DbResult<Vec<Metadata>> {
        let results = metadata::table.load::<Metadata>(&mut self.connection)?;
        Ok(results) 
    }

    // get total row count of metadata
    pub fn get_metadata_count(&mut self) -> DbResult<i64> {
        let count: i64 = metadata::table.count().get_result(&mut self.connection)?;
        Ok(count)
    }

    // get all market entries
    pub fn get_all_market(&mut self) -> DbResult<Vec<Market>> {
        let results = market::table.load::<Market>(&mut self.connection)?;
        Ok(results)
    }

    // get total row count of market
    pub fn get_market_count(&mut self) -> DbResult<i64> {
        let count: i64 = market::table.count().get_result(&mut self.connection)?;
        Ok(count)
    }

    // find metadata by title
    pub fn find_metadata_by_title(&mut self, title: &str) -> DbResult<Option<Metadata>> {
        let result = metadata::table
            .filter(metadata::title.eq(title))
            .first::<Metadata>(&mut self.connection)
            .optional()?;
        Ok(result)
    }

    // find market by mkcode
    pub fn find_market_by_mkcode(&mut self, mkcode: &str) -> DbResult<Option<Market>> {
        let result = market::table
            .filter(market::mkcode.eq(mkcode))
            .first::<Market>(&mut self.connection)
            .optional()?;
        Ok(result)
    }

    // set lastvisit by market id
    pub fn update_market_lastvisit(&mut self, mkcode: &str, lastvisit: i64) -> DbResult<usize> {
        let rows_updated = diesel::update(market::table.filter(market::mkcode.eq(mkcode)))
            .set(market::lastvisit.eq(lastvisit))
            .execute(&mut self.connection)?;
        Ok(rows_updated)    
    }
}