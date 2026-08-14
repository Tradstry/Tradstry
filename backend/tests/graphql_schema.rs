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

    let sync_result = sdl
        .split_once("type SyncResult {")
        .and_then(|(_, tail)| tail.split_once('}'))
        .map(|(fields, _)| fields)
        .expect("the brokerage sync result type must be present");
    assert!(
        sync_result.contains("status: String!"),
        "the sync mutation response must expose its completion status"
    );
    assert!(
        sdl.contains("brokerageSyncOutcome(workspaceId: String!): BrokerageSyncOutcome"),
        "the delayed sync outcome query must be present"
    );
    assert!(sdl.contains("type BrokerageSyncOutcome {"));
    assert!(sdl.contains("error: String"));
    assert!(sdl.contains("diagnosticId: String"));
    assert!(sdl.contains("succeededAt: String"));
    assert!(sdl.contains("transactionsSynced: Int!"));
    assert!(sdl.contains("holdingsSynced: Int!"));
    assert!(sdl.contains("balancesSynced: Int!"));
    assert!(sdl.contains("brokerageReconciliation(workspaceId: String!): BrokerageReconciliation"));
    assert!(sdl.contains("type BrokerageReconciliation {"));
    assert!(sdl.contains("brokerTransactionCount: Int!"));
    assert!(sdl.contains("localTransactionCount: Int!"));
    assert!(sdl.contains("missingTransactionCount: Int!"));
    assert!(sdl.contains("balanceDiscrepancyCount: Int!"));
    assert!(sdl.contains("transactionError: String"));
    assert!(sdl.contains("portfolioError: String"));
    assert!(
        sdl.contains("tranches: [HistoryTranche!]!"),
        "calculator history must expose its resolved execution legs"
    );
    assert!(
        sdl.contains("tranches: [CreateHistoryTrancheInput!]"),
        "calculator history creation must accept an execution snapshot"
    );
    assert!(
        sdl.contains("manualExecutionClaims(workspaceId: String!): [ManualExecutionClaimGql!]!")
    );
    assert!(sdl.contains("recordManualExecution("));
    assert!(sdl.contains("dismissManualExecution(id: String!): Boolean!"));
    assert!(sdl.contains(
        "publishBrokerageEpisodeReview(input: PublishBrokerageEpisodeReviewInput!): String!"
    ));
    assert!(sdl.contains("requiresManualGrouping: Boolean!"));
    assert!(sdl.contains("isManuallyGrouped: Boolean!"));
    assert!(sdl.contains("episodeId: String!"));
    assert!(sdl.contains(
        "regroupBrokerageEpisode(episodeId: String!, transactionIds: [String!]!): String!"
    ));
    assert!(sdl.contains("resetBrokerageEpisodeGrouping(episodeId: String!): Boolean!"));
}
