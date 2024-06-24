use bb8::PooledConnection;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use serde_json::json;
use crate::{CreateSession, Review, ShortenRequest};
use anyhow::Result;

pub async fn save_review<'a>(
    mut conn: PooledConnection<'a, RedisConnectionManager>,
    review: &Review,
) -> Result<()> {
    let key = format!("reviews/{}", review.id.to_string());
    conn.set::<&String, String, String>(&key, json!(review).to_string()).await?;
    conn.sadd::<&str, &String, i32>("reviews", &key).await?;

    Ok(())
}

pub async fn try_auth<'a>(
    mut conn: PooledConnection<'a, RedisConnectionManager>,
    auth: &CreateSession
) -> Result<()> { 
    let key = format!("auth/{}", auth.username);
    let hash = conn.get::<&str, String>(&key).await?;

    bcrypt::verify(auth.password.clone(), hash.as_str())?;

    Ok(())
}

pub async fn shorten_link<'a>(
    mut conn: PooledConnection<'a, RedisConnectionManager>,
    request: &ShortenRequest
) -> Result<()> {
    let key = format!("url/{}", request.short.clone().as_str());
    conn.set::<String, String, String>(key, request.url.clone()).await?;

    Ok(())
}

pub async fn get_link<'a>(
    mut conn: PooledConnection<'a, RedisConnectionManager>,
    short: String,
) -> Result<String> {
    let key = format!("url/{}", short.as_str());
    tracing::debug!(key);
    let long = conn.get::<String, String>(key).await?;

    Ok(long)
}