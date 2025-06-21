// @generated automatically by Diesel CLI.

diesel::table! {
    instances (node_id) {
        node_id -> Text,
        name -> Text,
        created_at -> BigInt,
    }
}
