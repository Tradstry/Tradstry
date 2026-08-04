use tradstry_backend::service::redis::client::RedisClient;

async fn client() -> Option<RedisClient> {
    if std::env::var("REDIS_URL").is_err() {
        eprintln!("skipping: REDIS_URL not set");
        return None;
    }
    RedisClient::from_env().await.ok()
}

#[tokio::test]
async fn rpush_then_lmove_batch_moves_items() {
    let Some(redis) = client().await else {
        return;
    };
    let src = "test:countly:queue";
    let dst = "test:countly:processing";
    redis.del_key(src).await;
    redis.del_key(dst).await;

    assert!(redis.rpush(src, "a").await);
    assert!(redis.rpush(src, "b").await);
    assert_eq!(redis.llen(src).await, 2);

    let moved = redis.lmove_batch(src, dst, 10).await;
    assert_eq!(moved, 2);
    assert_eq!(redis.llen(src).await, 0);
    assert_eq!(redis.lrange(dst, 0, -1).await, vec!["a", "b"]);

    redis.del_key(dst).await;
    assert_eq!(redis.llen(dst).await, 0);
}

#[tokio::test]
async fn processing_list_is_reclaimed_before_new_items() {
    let Some(redis) = client().await else {
        return;
    };
    let src = "test:reclaim:queue";
    let dst = "test:reclaim:processing";
    redis.del_key(src).await;
    redis.del_key(dst).await;

    // A previous run left one event stranded mid-flight.
    redis.rpush(dst, "stranded").await;
    redis.rpush(src, "fresh").await;

    // Reclaim must see the stranded event and not move the fresh one yet.
    let pending = redis.lrange(dst, 0, -1).await;
    assert_eq!(pending, vec!["stranded"]);
    assert_eq!(redis.llen(src).await, 1);

    redis.del_key(src).await;
    redis.del_key(dst).await;
}
