use async_graphql::Schema;
use tradstry_backend::graphql::{Mutation, Query, Subscription};

/// async-graphql panics while registering two types under one GraphQL name, and it
/// does so at schema build — which happens on server startup, not in any test. Two
/// distinct Rust types called `NotebookMutation` (the mutation root, and the push
/// mutation's input object) took the backend down on boot. Building the roots needs
/// none of the injected clients, so a plain test catches it.
#[test]
fn schema_builds_without_duplicate_type_names() {
    let schema = Schema::build(
        Query::default(),
        Mutation::default(),
        Subscription::default(),
    )
    .finish();

    let sdl = schema.sdl();
    assert!(
        sdl.contains("input NotebookMutationInput"),
        "the push mutation's input object must not collide with the mutation root"
    );
    assert!(sdl.contains("marketQuotes(symbols: [String!]!): MarketQuotesGql!"));
    assert!(
        sdl.contains("marketPriceUpdates(symbols: [String!]!): MarketPriceUpdateGql!"),
        "the live market subscription must be present"
    );
}
