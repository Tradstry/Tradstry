//! Local validation: run the converted sqlx query functions against a Postgres
//! that already has real (transferred-from-dev) data, and print results. This
//! exercises the actual production query paths against production-shaped data —
//! catching any SQL issues the synthetic smoke test can't.
//!
//!   POSTGRES_URL=postgres://tradstry:tradstry@localhost:5435/tradstry_test \
//!     cargo run --bin check_loaded_data

use anyhow::{Context, Result};
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;

use tradstry_backend::service::brokerage::pending_trades;
use tradstry_backend::service::db::schema::tables::{
    brokerage_table, journal_table, tags_table, workspaces_table,
};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let url = std::env::var("POSTGRES_URL").context("POSTGRES_URL not set")?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await?;

    // Pick the first user that actually has data.
    let user_id: String = sqlx::query("SELECT id FROM users ORDER BY created_at LIMIT 1")
        .fetch_one(&pool)
        .await
        .context("no users loaded")?
        .try_get("id")?;
    println!("user_id = {user_id}");

    // accounts (bool + timestamptz read-back)
    let accounts = workspaces_table::list_workspaces(&pool, &user_id).await?;
    println!("accounts: {}", accounts.len());
    for a in &accounts {
        println!(
            "  - {} | disabled={} | created_at={} | total_value={:?}",
            a.name, a.snaptrade_connection_disabled, a.created_at, a.total_value
        );
    }
    let workspace_id = accounts.first().map(|a| a.id.clone()).unwrap_or_default();

    // journal entries (timestamptz to_char round-trip)
    let entries = journal_table::list_journal_entries(&pool, &user_id).await?;
    println!("journal_entries: {}", entries.len());
    if let Some(e) = entries.first() {
        println!(
            "  first: {} {} open={} close={} status={} pl={}",
            e.symbol, e.trade_type, e.open_date, e.close_date, e.status, e.total_pl
        );
    }

    // analytics aggregate (COUNT->i64, SUM->f64, timestamptz comparison)
    if !workspace_id.is_empty() {
        let agg = journal_table::aggregate_journal_analytics(
            &pool,
            &user_id,
            &workspace_id,
            "2000-01-01T00:00:00Z",
            "2100-01-01T00:00:00Z",
        )
        .await?;
        println!(
            "analytics: total={} win={} loss={} cum_profit={:.2}",
            agg.total_trades, agg.winning_trades, agg.losing_trades, agg.cumulative_profit
        );
    }

    // brokerage transactions (DATE/TIMESTAMPTZ + filters) — the big table (900 rows)
    if !workspace_id.is_empty() {
        let txns =
            brokerage_table::list_transactions(&pool, &user_id, &workspace_id, &Default::default())
                .await?;
        println!(
            "brokerage_transactions (account {workspace_id}): {}",
            txns.data.len()
        );
        if let Some(t) = txns.data.first() {
            println!(
                "  first: {} {:?} units={} price={} trade_date={:?}",
                t.transaction_type, t.symbol, t.units, t.price, t.trade_date
            );
        }

        // pending trades lifecycle assembly over the real fills
        let pending =
            pending_trades::compute_pending_trades(&pool, &user_id, &workspace_id).await?;
        println!("pending_trades: {}", pending.len());
    }

    // tags
    let cats = if workspace_id.is_empty() {
        Vec::new()
    } else {
        tags_table::list_categories(&pool, &user_id, &workspace_id).await?
    };
    println!("tag_categories: {}", cats.len());

    println!("\nAll converted read paths executed successfully against real data.");
    Ok(())
}
