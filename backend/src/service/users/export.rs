use anyhow::Result;
use serde_json::{Map, Value};
use sqlx::PgPool;

/// `(key, sql)` pairs rather than interpolated table names: sqlx 0.9 only accepts
/// `&'static str` queries, which also makes the table list injection-proof by construction.
const USER_SCOPED: &[(&str, &str)] = &[
    (
        "workspaces",
        "SELECT to_jsonb(t) FROM workspaces t WHERE t.user_id = $1",
    ),
    (
        "brokerage_connections",
        "SELECT to_jsonb(t) - 'snaptrade_user_secret_encrypted' FROM brokerage_connections t WHERE t.user_id = $1",
    ),
    (
        "journal_entries",
        "SELECT to_jsonb(t) FROM journal_entries t WHERE t.user_id = $1",
    ),
    (
        "playbooks",
        "SELECT to_jsonb(t) FROM playbooks t WHERE t.user_id = $1",
    ),
    (
        "trading_principles",
        "SELECT to_jsonb(t) FROM trading_principles t WHERE t.user_id = $1",
    ),
    (
        "tags",
        "SELECT to_jsonb(t) FROM tags t WHERE t.user_id = $1",
    ),
    (
        "tag_categories",
        "SELECT to_jsonb(t) FROM tag_categories t WHERE t.user_id = $1",
    ),
    (
        "notebook_folders",
        "SELECT to_jsonb(t) FROM notebook_folders t WHERE t.user_id = $1",
    ),
    (
        "notebook_notes",
        "SELECT to_jsonb(t) FROM notebook_notes t WHERE t.user_id = $1",
    ),
    (
        "notebook_images",
        "SELECT to_jsonb(t) FROM notebook_images t WHERE t.user_id = $1",
    ),
    (
        "brokerage_transactions",
        "SELECT to_jsonb(t) FROM brokerage_transactions t WHERE t.user_id = $1",
    ),
    (
        "brokerage_holdings",
        "SELECT to_jsonb(t) FROM brokerage_holdings t WHERE t.user_id = $1",
    ),
    (
        "brokerage_balances",
        "SELECT to_jsonb(t) FROM brokerage_balances t WHERE t.user_id = $1",
    ),
    (
        "journal_brokerage_links",
        "SELECT to_jsonb(t) FROM journal_brokerage_links t WHERE t.user_id = $1",
    ),
    (
        "account_equity_history",
        "SELECT to_jsonb(t) FROM account_equity_history t WHERE t.user_id = $1",
    ),
    (
        "position_calculator_rules",
        "SELECT to_jsonb(t) FROM position_calculator_rules t WHERE t.user_id = $1",
    ),
    (
        "position_calculator_history",
        "SELECT to_jsonb(t) FROM position_calculator_history t WHERE t.user_id = $1",
    ),
    (
        "position_calculator_plans",
        "SELECT to_jsonb(t) FROM position_calculator_plans t WHERE t.user_id = $1",
    ),
    (
        "user_agents",
        "SELECT to_jsonb(t) FROM user_agents t WHERE t.user_id = $1",
    ),
    (
        "user_prompts",
        "SELECT to_jsonb(t) FROM user_prompts t WHERE t.user_id = $1",
    ),
];

/// Junction tables carry no user_id, so each needs the join that reaches one. Without
/// these the export silently omits which tags and principles every trade was marked with.
const JOINED: &[(&str, &str)] = &[
    (
        "trade_tags",
        "SELECT to_jsonb(t) FROM trade_tags t
         JOIN journal_entries j ON j.id = t.journal_entry_id
         WHERE j.user_id = $1",
    ),
    (
        "trade_principle_violations",
        "SELECT to_jsonb(v) FROM trade_principle_violations v
         JOIN journal_entries j ON j.id = v.journal_entry_id
         WHERE j.user_id = $1",
    ),
    (
        "notebook_note_trades",
        "SELECT to_jsonb(nt) FROM notebook_note_trades nt
         JOIN notebook_notes n ON n.id = nt.note_id
         WHERE n.user_id = $1",
    ),
];

async fn fetch(pool: &PgPool, sql: &'static str, user_id: &str) -> Result<Value> {
    let rows = sqlx::query_scalar::<_, Value>(sql)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(Value::Array(rows))
}

pub async fn build_export(pool: &PgPool, user_id: &str) -> Result<Value> {
    let mut out = Map::new();

    let user = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(u) FROM users u WHERE u.id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    out.insert("user".into(), user.unwrap_or(Value::Null));

    for (key, sql) in USER_SCOPED.iter().chain(JOINED) {
        out.insert((*key).into(), fetch(pool, sql, user_id).await?);
    }

    Ok(Value::Object(out))
}
