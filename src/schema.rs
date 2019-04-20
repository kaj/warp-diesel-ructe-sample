table! {
    sessions (id) {
        id -> Int4,
        cookie -> Varchar,
        user_id -> Int4,
    }
}

table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        realname -> Varchar,
        password -> Varchar,
    }
}

joinable!(sessions -> users (user_id));

allow_tables_to_appear_in_same_query!(sessions, users);
