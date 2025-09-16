// @generated automatically by Diesel CLI.

diesel::table! {
    metadata (id) {
        id -> Integer,
        blacklisted -> Bool,
        fullstartdate -> Text,
        image_id -> Text,
        title -> Text,
        author -> Text,
        description -> Text,
        copyright -> Text,
        copyright_link -> Text,
        thumbnail_url -> Text,
        full_url -> Text,
    }
}

diesel::table! {
    market (id) {
        id -> Integer,
        mkcode -> Text,
        lastvisit -> BigInt,
    }
}

diesel::allow_tables_to_appear_in_same_query!(metadata, market,);