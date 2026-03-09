prismar::diesel::table! {
    namespaces (name) {
        name -> Text,
    }
}

prismar::diesel::table! {
    cargoes (key) {
        key -> Text,
        created_at -> Timestamp,
        name -> Text,
        spec_key -> Text,
        status_key -> Text,
        namespace_name -> Text,
    }
}

prismar::diesel::joinable!(cargoes -> namespaces (namespace_name));

prismar::diesel::allow_tables_to_appear_in_same_query!(
    namespaces,
    cargoes,
);
