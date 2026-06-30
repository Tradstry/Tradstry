use log::info;

use crate::service::db::schema::tables::brokerage_table::{
    BrokerageBalance, BrokerageHolding, BrokerageTransaction, TransactionPage,
};
use crate::service::redis::client::RedisClient;

/// Cache TTL: 10 minutes (safety net; primary invalidation is explicit).
const CACHE_TTL: u64 = 600;

/// Build a cache key for a paginated transactions query.
/// Hash the filter params so each unique query gets its own entry.
#[allow(clippy::too_many_arguments)]
fn tx_cache_key(
    user_id: &str,
    account_id: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    transaction_type: Option<&str>,
    symbol: Option<&str>,
    sort_by: Option<&str>,
    is_journalled: Option<bool>,
    offset: i32,
    limit: i32,
) -> String {
    let raw = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        start_date.unwrap_or(""),
        end_date.unwrap_or(""),
        transaction_type.unwrap_or(""),
        symbol.unwrap_or(""),
        sort_by.unwrap_or(""),
        match is_journalled {
            None => "all",
            Some(true) => "journalled",
            Some(false) => "unjournalled",
        },
        offset,
        limit,
    );
    let hash = format!("{:x}", md5::compute(raw.as_bytes()));
    format!("brokerage:tx:{user_id}:{account_id}:{hash}")
}

fn holdings_cache_key(user_id: &str, account_id: &str) -> String {
    format!("brokerage:hold:{user_id}:{account_id}")
}

fn balances_cache_key(user_id: &str, account_id: &str) -> String {
    format!("brokerage:bal:{user_id}:{account_id}")
}

// ── Transactions ───────────────────────────────────────────────────────────

/// Try cache first. On miss, call `load` to fetch from DB, cache the result, return it.
#[allow(clippy::too_many_arguments)]
pub async fn get_or_load_transactions<F, Fut>(
    redis: &RedisClient,
    user_id: &str,
    account_id: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
    transaction_type: Option<&str>,
    symbol: Option<&str>,
    sort_by: Option<&str>,
    is_journalled: Option<bool>,
    offset: i32,
    limit: i32,
    load: F,
) -> anyhow::Result<TransactionPage>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<TransactionPage>>,
{
    let key = tx_cache_key(
        user_id,
        account_id,
        start_date,
        end_date,
        transaction_type,
        symbol,
        sort_by,
        is_journalled,
        offset,
        limit,
    );

    // Try cache
    if let Some(cached) = redis.get(&key).await
        && let Ok(page) = serde_json::from_str::<CachedTransactionPage>(&cached)
    {
        return Ok(TransactionPage {
            data: page.data,
            total: page.total,
            offset: page.offset,
            limit: page.limit,
        });
    }

    // Cache miss — load from DB
    let page = load().await?;

    // Cache the result (fire-and-forget, errors are logged inside set_ex)
    let cached = CachedTransactionPage {
        data: page.data.clone(),
        total: page.total,
        offset: page.offset,
        limit: page.limit,
    };
    if let Ok(json) = serde_json::to_string(&cached) {
        redis.set_ex(&key, &json, CACHE_TTL).await;
    }

    Ok(page)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedTransactionPage {
    data: Vec<BrokerageTransaction>,
    total: i32,
    offset: i32,
    limit: i32,
}

// ── Holdings ───────────────────────────────────────────────────────────────

pub async fn get_or_load_holdings<F, Fut>(
    redis: &RedisClient,
    user_id: &str,
    account_id: &str,
    load: F,
) -> anyhow::Result<Vec<BrokerageHolding>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Vec<BrokerageHolding>>>,
{
    let key = holdings_cache_key(user_id, account_id);

    if let Some(cached) = redis.get(&key).await
        && let Ok(holdings) = serde_json::from_str::<Vec<BrokerageHolding>>(&cached)
    {
        return Ok(holdings);
    }

    let holdings = load().await?;

    if let Ok(json) = serde_json::to_string(&holdings) {
        redis.set_ex(&key, &json, CACHE_TTL).await;
    }

    Ok(holdings)
}

// ── Balances ───────────────────────────────────────────────────────────────

pub async fn get_or_load_balances<F, Fut>(
    redis: &RedisClient,
    user_id: &str,
    account_id: &str,
    load: F,
) -> anyhow::Result<Vec<BrokerageBalance>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Vec<BrokerageBalance>>>,
{
    let key = balances_cache_key(user_id, account_id);

    if let Some(cached) = redis.get(&key).await
        && let Ok(balances) = serde_json::from_str::<Vec<BrokerageBalance>>(&cached)
    {
        return Ok(balances);
    }

    let balances = load().await?;

    if let Ok(json) = serde_json::to_string(&balances) {
        redis.set_ex(&key, &json, CACHE_TTL).await;
    }

    Ok(balances)
}

// ── Invalidation ───────────────────────────────────────────────────────────

/// Delete all cached brokerage data for a given user+account.
pub async fn invalidate_account_cache(redis: &RedisClient, user_id: &str, account_id: &str) {
    info!(
        "[redis] Invalidating brokerage cache for user={}, account={}",
        user_id, account_id
    );
    redis
        .delete_by_prefix(&format!("brokerage:*:{user_id}:{account_id}*"))
        .await;
}
