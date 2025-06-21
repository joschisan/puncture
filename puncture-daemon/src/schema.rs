// @generated automatically by Diesel CLI.

diesel::table! {
    bolt11_invoice (payment_hash) {
        payment_hash -> Text,
        user_pk -> Text,
        amount_msat -> BigInt,
        description -> Text,
        invoice -> Text,
        expires_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    bolt11_receive (payment_hash) {
        payment_hash -> Text,
        user_pk -> Text,
        amount_msat -> BigInt,
        description -> Text,
        invoice -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    bolt11_send (payment_hash) {
        payment_hash -> Text,
        user_pk -> Text,
        amount_msat -> BigInt,
        fee_msat -> BigInt,
        description -> Text,
        invoice -> Text,
        status -> Text,
        ln_address -> Nullable<Text>,
        created_at -> BigInt,
    }
}

diesel::table! {
    users (user_pk) {
        user_pk -> Text,
        created_at -> BigInt,
    }
}

diesel::allow_tables_to_appear_in_same_query!(bolt11_invoice, bolt11_receive, bolt11_send, users,);
