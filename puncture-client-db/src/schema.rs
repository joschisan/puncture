// @generated automatically by Diesel CLI.

diesel::table! {
    daemon (node_id) {
        node_id -> Text,
        network -> Text,
        name -> Text,
        created_at -> BigInt,
    }
}
