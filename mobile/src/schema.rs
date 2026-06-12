// @generated automatically by Diesel CLI.

diesel::table! {
    bing_images (id) {
        id -> Integer,
        url -> Text,
        title -> Text,
        copyright -> Nullable<Text>,
        copyright_link -> Nullable<Text>,
        market_code -> Text,
        fetched_at -> Integer,
        status -> Text,
        created_at -> Integer,
        updated_at -> Integer
    }
}

diesel::table! {
    config_kv (id) {
        id -> Integer,
        key -> Text,
        value -> Text,
        created_at -> Integer,
        updated_at -> Integer
    }
}

diesel::table! {
    market_codes (id) {
        id -> Integer,
        code -> Text,
        last_used_at -> Integer,
        created_at -> Integer,
        updated_at -> Integer
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    bing_images,
    config_kv,
    market_codes,
);
