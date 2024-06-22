use bb8::PooledConnection;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use serde_json::json;
use crate::Review;

pub async fn save_review<'a>(
    mut conn: PooledConnection<'a, RedisConnectionManager>,
    review: &Review,
) -> Result<(), anyhow::Error> {
    let key = format!("reviews/{}", review.id.to_string());
    conn.set::<&String, String, String>(&key, json!(review).to_string()).await?;
    conn.sadd::<&str, &String, i32>("reviews", &key).await?;

    Ok(())
}