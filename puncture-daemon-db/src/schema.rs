// @generated automatically by Diesel CLI.

diesel::table! {
    invite (id) {
        id -> Text,
        user_limit -> BigInt,
        expires_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    invoice (id) {
        id -> Text,
        user_pk -> Text,
        amount_msat -> Nullable<BigInt>,
        description -> Text,
        pr -> Text,
        expires_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    receive (id) {
        id -> Text,
        user_pk -> Text,
        amount_msat -> BigInt,
        description -> Text,
        pr -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    send (id) {
        id -> Text,
        user_pk -> Text,
        amount_msat -> BigInt,
        fee_msat -> BigInt,
        description -> Text,
        pr -> Text,
        status -> Text,
        ln_address -> Nullable<Text>,
        created_at -> BigInt,
    }
}

diesel::table! {
    offer (id) {
        id -> Text,
        user_pk -> Text,
        amount_msat -> Nullable<BigInt>,
        description -> Text,
        pr -> Text,
        expires_at -> Nullable<BigInt>,
        created_at -> BigInt,
    }
}

diesel::table! {
    recovery (id) {
        id -> Text,
        user_pk -> Text,
        expires_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    user (user_pk) {
        user_pk -> Text,
        invite_id -> Text,
        recovery_name -> Nullable<Text>,
        created_at -> BigInt,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    invite, invoice, receive, send, offer, recovery, user,
);
