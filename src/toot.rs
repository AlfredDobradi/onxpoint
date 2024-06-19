use std::collections::HashMap;
use std::env;
use anyhow::Result;
use axum::http::{HeaderMap, HeaderValue};
use serde_json::Value;

pub async fn create_toot(review: crate::Review) -> Result<Value> {
    let debug_mode = env::var("OXP_DEBUG")?;
    let mastodon_host = env::var("OXP_MASTODON_HOST")?;
    let mut access_token = env::var("OXP_ACCESS_TOKEN")?;
    access_token.insert_str(0, "Bearer ");

    // tracing::info!("access_token_header: {}", access_token_header);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Authorization", HeaderValue::from_str(access_token.as_str())?);

    let r = format!("{}\nSpotify: {}", review.review, review.url);

    let mut payload: HashMap<&str, &str> = HashMap::new();
    payload.insert("status", r.as_str());

    if debug_mode == "1" {
        payload.insert("visibility", "private");
    }

    if review.schedule != "" {
        payload.insert("scheduled_at", review.schedule.as_str());
    }

    let client = reqwest::Client::new();
    let api_endpoint = format!("{}/api/v1/statuses", mastodon_host);
    let res = client.post(api_endpoint)
        .headers(headers)
        .json(&payload)
        .send()
        .await?
        .json::<Value>()
        .await?;

    Ok(res)
}