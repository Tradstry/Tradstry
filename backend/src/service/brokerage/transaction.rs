use anyhow::{Context, Result};

use sqlx::PgPool;

use super::client::{
    BrokerageClient, SnapTradeActivity, SnapTradeOptionPosition, SnapTradeOptionSymbol,
    SnapTradePosition, TransactionsSyncStatus,
};
use crate::service::db::schema::tables::brokerage_table::{
    self, NewBrokerageBalance, NewBrokerageHolding, NewBrokerageTransaction,
};

// Sync deliberately requests the account's full available history every run
// rather than reading forward from a watermark.
//
// A MAX(trade_date) watermark is a hard floor: a fill the brokerage backdates or
// amends after the fact lands below it and is never fetched again, silently
// missing forever. Any fixed lookback just moves that floor. Re-reading
// everything is safe because the upsert keys on `dedup_key`, which is derived
// from the trade's own attributes — the overlap updates rows in place instead of
// duplicating them, and it re-keys automatically after a re-registration.
//
// The cost is bounded by the account's history, not by elapsed time, and is paid
// in one batched write per page rather than per fill.

/// Fetches transactions only when SnapTrade reports it has synced further than
/// we last saw, returning `None` when the fetch was skipped.
///
/// `remote` is `sync_status.transactions` from the list-accounts response we
/// already make, so the check costs no extra request. SnapTrade advances
/// `last_successful_sync` at most once a day, which keeps us within their "no
/// more than once per account per 24h" guidance for this endpoint.
///
/// Fails open: a missing or unparseable status syncs rather than stalling
/// forever on a signal we cannot read.
pub async fn sync_transactions_if_advanced(
    client: &BrokerageClient,
    pool: &PgPool,
    user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_account_id: &str,
    remote: Option<&TransactionsSyncStatus>,
) -> Result<Option<u64>> {
    // History is still being backfilled; syncing now would store a partial
    // picture and the next advance will pick it up anyway.
    if let Some(status) = remote
        && status.initial_sync_completed == Some(false)
    {
        log::info!(
            "SnapTrade initial transaction sync still running for st_account={snaptrade_account_id}; skipping"
        );
        return Ok(None);
    }

    let remote_mark = remote.and_then(|s| s.last_successful_sync.as_deref());

    if let Some(remote_mark) = remote_mark {
        let local_mark = brokerage_table::transactions_synced_through(
            pool,
            user_id,
            internal_account_id,
            snaptrade_account_id,
        )
        .await?;

        // Day-granular ISO dates, so lexical ordering is chronological.
        if local_mark.as_deref() >= Some(remote_mark) {
            log::info!(
                "SnapTrade has no new transactions for st_account={snaptrade_account_id} \
                 (synced through {remote_mark}); skipping fetch"
            );
            return Ok(None);
        }
    }

    let synced = sync_transactions(
        client,
        pool,
        user_id,
        user_secret,
        snaptrade_account_id,
        internal_account_id,
    )
    .await?;

    // Recorded only after a successful fetch, so a failure mid-sync leaves the
    // account stale and it retries next run rather than skipping ahead.
    if let Some(remote_mark) = remote_mark {
        brokerage_table::record_transactions_synced_through(
            pool,
            user_id,
            internal_account_id,
            snaptrade_account_id,
            remote_mark,
        )
        .await?;
    }

    Ok(Some(synced))
}

/// Syncs transactions from SnapTrade.
/// `snaptrade_account_id` is the SnapTrade-side account ID (for API calls).
/// `internal_account_id` is the Tradstry-side account ID (for DB storage).
pub async fn sync_transactions(
    client: &BrokerageClient,
    pool: &PgPool,
    user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_account_id: &str,
) -> Result<u64> {
    let mut total_synced = 0u64;
    let mut offset = 0i32;
    let limit = 1000i32;
    // Shared across pages so an order's identical partial fills keep distinct
    // ordinals even when the page boundary falls between them.
    let mut seen = brokerage_table::SignatureCounts::new();

    log::info!("Full-history sync for account={internal_account_id}");

    loop {
        let response = client
            .fetch_transactions(
                user_id,
                user_secret,
                snaptrade_account_id,
                None,
                None,
                None,
                Some(offset),
                Some(limit),
            )
            .await
            .context("Failed to fetch transactions page")?;

        let activities = &response.data;
        if activities.is_empty() {
            break;
        }

        let new_txs: Vec<NewBrokerageTransaction> = activities
            .iter()
            .filter_map(map_activity_to_transaction)
            .collect();

        let upserted = brokerage_table::upsert_transactions(
            pool,
            user_id,
            internal_account_id,
            &new_txs,
            &mut seen,
        )
        .await
        .context("Failed to upsert transactions")?;
        total_synced += upserted;

        let page_total = response
            .pagination
            .as_ref()
            .and_then(|p| p.total)
            .unwrap_or(activities.len() as i32);

        offset += limit;
        if offset >= page_total {
            break;
        }
    }

    if total_synced > 0
        && let Err(e) = crate::service::equity::rebuild::rebuild_account_equity(
            pool,
            user_id,
            internal_account_id,
        )
        .await
    {
        log::warn!("equity: rebuild after transaction sync failed: {e}");
    }

    Ok(total_synced)
}

/// Syncs holdings from SnapTrade.
/// `snaptrade_account_id` is the SnapTrade-side account ID (for API calls).
/// `internal_account_id` is the Tradstry-side account ID (for DB storage).
pub async fn sync_holdings(
    client: &BrokerageClient,
    pool: &PgPool,
    user_id: &str,
    user_secret: &str,
    snaptrade_account_id: &str,
    internal_account_id: &str,
) -> Result<(u64, u64)> {
    let response = client
        .fetch_holdings(user_id, user_secret, snaptrade_account_id)
        .await
        .context("Failed to fetch holdings")?;

    let mut holdings: Vec<NewBrokerageHolding> = Vec::new();

    if let Some(positions) = &response.positions {
        for p in positions {
            if let Some(h) = map_position_to_holding(p) {
                holdings.push(h);
            }
        }
    }

    if let Some(option_positions) = &response.option_positions {
        for op in option_positions {
            if let Some(h) = map_option_position_to_holding(op) {
                holdings.push(h);
            }
        }
    }

    let holdings_count =
        brokerage_table::replace_holdings(pool, user_id, internal_account_id, &holdings).await?;

    let balances: Vec<NewBrokerageBalance> = response
        .balances
        .as_ref()
        .map(|bals| {
            bals.iter()
                .map(|b| NewBrokerageBalance {
                    currency: b
                        .currency
                        .as_ref()
                        .and_then(|c| c.code.clone())
                        .unwrap_or_else(|| "USD".to_string()),
                    cash: b.cash,
                    buying_power: b.buying_power,
                })
                .collect()
        })
        .unwrap_or_default();

    let balances_count =
        brokerage_table::replace_balances(pool, user_id, internal_account_id, &balances).await?;

    // Persist SnapTrade's authoritative total market value (account.balance.total)
    // on the account row so analytics can compute true equity-based drawdown. Only
    // write when an amount is present — None leaves the column null.
    if let Some(tv) = &response.total_value
        && let Some(amount) = tv.amount
    {
        crate::service::db::schema::tables::accounts_table::update_total_value(
            pool,
            internal_account_id,
            user_id,
            amount,
            tv.currency.as_deref(),
        )
        .await?;
    }

    Ok((holdings_count, balances_count))
}

fn map_activity_to_transaction(a: &SnapTradeActivity) -> Option<NewBrokerageTransaction> {
    let snaptrade_id = a.id.clone()?;
    let settlement_date = a.settlement_date.clone().unwrap_or_default();
    let institution = a.institution.clone().unwrap_or_default();
    let transaction_type = a.activity_type.clone().unwrap_or_default();
    let raw_json = serde_json::to_string(a).unwrap_or_default();

    // For option activities the top-level `symbol` is null and the contract
    // details arrive under `option_symbol`. Equities leave `opt` as None.
    let opt = a.option_symbol.as_ref().and_then(|v| {
        if v.is_null() {
            None
        } else {
            serde_json::from_value::<SnapTradeOptionSymbol>(v.clone()).ok()
        }
    });

    let (
        symbol,
        symbol_description,
        contract_multiplier,
        underlying_symbol,
        option_kind,
        strike_price,
        option_expiration,
    ) = match &opt {
        Some(o) => {
            let underlying = o.underlying_symbol.as_ref().and_then(|s| s.symbol.clone());
            let option_kind = o.option_type.clone();
            let description = format_option_description(
                underlying.as_deref(),
                option_kind.as_deref(),
                o.strike_price,
                o.expiration_date.as_deref(),
            );
            (
                o.ticker.clone(),
                Some(description),
                if o.is_mini_option.unwrap_or(false) {
                    10.0
                } else {
                    100.0
                },
                underlying,
                option_kind,
                o.strike_price,
                o.expiration_date.clone(),
            )
        }
        None => (
            a.symbol.as_ref().and_then(|s| s.symbol.clone()),
            a.symbol.as_ref().and_then(|s| s.description.clone()),
            1.0,
            None,
            None,
            None,
            None,
        ),
    };

    Some(NewBrokerageTransaction {
        snaptrade_id,
        symbol,
        symbol_description,
        raw_symbol: a.symbol.as_ref().and_then(|s| s.raw_symbol.clone()),
        currency: a
            .currency
            .as_ref()
            .and_then(|c| c.code.clone())
            .unwrap_or_else(|| "USD".to_string()),
        transaction_type,
        option_type: a.option_type.clone(),
        price: a.price.unwrap_or(0.0),
        units: a.units.unwrap_or(0.0),
        amount: a.amount,
        fee: a.fee.unwrap_or(0.0),
        fx_rate: a.fx_rate,
        description: a.description.clone(),
        trade_date: a.trade_date.clone(),
        settlement_date,
        institution,
        external_reference_id: a.external_reference_id.clone(),
        raw_json,
        contract_multiplier,
        underlying_symbol,
        option_kind,
        strike_price,
        option_expiration,
    })
}

/// Human-readable option contract label, e.g. `AAPL $150 Call 2026-01-17`.
/// Missing parts are skipped so a partial contract still renders cleanly.
fn format_option_description(
    underlying: Option<&str>,
    kind: Option<&str>,
    strike: Option<f64>,
    expiration: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(u) = underlying {
        parts.push(u.to_string());
    }
    if let Some(s) = strike {
        parts.push(format!("${s}"));
    }
    if let Some(k) = kind {
        let pretty = match k.to_ascii_uppercase().as_str() {
            "CALL" => "Call".to_string(),
            "PUT" => "Put".to_string(),
            other => other.to_string(),
        };
        parts.push(pretty);
    }
    if let Some(e) = expiration {
        parts.push(e.to_string());
    }
    parts.join(" ")
}

fn map_position_to_holding(p: &SnapTradePosition) -> Option<NewBrokerageHolding> {
    let symbol = p.symbol.as_ref().and_then(|s| s.symbol.clone())?;
    let raw_json = serde_json::to_string(p).unwrap_or_default();

    Some(NewBrokerageHolding {
        snaptrade_symbol_id: p.symbol.as_ref().and_then(|s| s.id.clone()),
        symbol,
        symbol_description: p.symbol.as_ref().and_then(|s| s.description.clone()),
        raw_symbol: p.symbol.as_ref().and_then(|s| s.raw_symbol.clone()),
        currency: p
            .currency
            .as_ref()
            .and_then(|c| c.code.clone())
            .unwrap_or_else(|| "USD".to_string()),
        units: p.units.unwrap_or(0.0),
        price: p.price.unwrap_or(0.0),
        market_value: None,
        open_pnl: p.open_pnl,
        average_purchase_price: p.average_purchase_price,
        is_option: false,
        option_type: None,
        strike_price: None,
        expiration_date: None,
        raw_json,
    })
}

fn map_option_position_to_holding(op: &SnapTradeOptionPosition) -> Option<NewBrokerageHolding> {
    let opt_sym = op.option_symbol.as_ref()?;
    let symbol = opt_sym.ticker.clone().unwrap_or_default();
    if symbol.is_empty() {
        return None;
    }
    let raw_json = serde_json::to_string(op).unwrap_or_default();

    Some(NewBrokerageHolding {
        snaptrade_symbol_id: opt_sym.id.clone(),
        symbol,
        symbol_description: opt_sym
            .underlying_symbol
            .as_ref()
            .and_then(|s| s.description.clone()),
        raw_symbol: opt_sym
            .underlying_symbol
            .as_ref()
            .and_then(|s| s.raw_symbol.clone()),
        currency: "USD".to_string(),
        units: op.units.unwrap_or(0.0),
        price: op.price.unwrap_or(0.0),
        market_value: None,
        open_pnl: None,
        average_purchase_price: op.average_purchase_price,
        is_option: true,
        option_type: opt_sym.option_type.clone(),
        strike_price: opt_sym.strike_price,
        expiration_date: opt_sym.expiration_date.clone(),
        raw_json,
    })
}
