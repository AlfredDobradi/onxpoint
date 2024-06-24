mod toot;
mod redict;
mod auth;
mod handler;
mod model;
mod error;

use std::collections::HashMap;

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::{
        header::HeaderMap,
        StatusCode
    },
    response::{Redirect, Response},
    routing::{get, post},
    Json,
    Router
};
use axum::response::IntoResponse;
use bb8_redis::RedisConnectionManager;
use serde::{Serialize, Deserialize};
use serde_json::json;
use uuid::Uuid;
use redis::AsyncCommands;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let redis_host = match std::env::var("OXP_REDIS_HOST") {
        Ok(host) => host,
        Err(e) => { panic!("OXP_REDIS_HOST: {}", e); }
    };

    // initialize tracing
    tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "onxpoint=debug".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

    // connect to redis
    let manager = RedisConnectionManager::new(redis_host)?;
    let pool = bb8::Pool::builder().build(manager).await?;

    {
        // ping the database before starting
        let mut conn = pool.get().await.unwrap();
        conn.set::<&str, &str, ()>("foo", "bar").await.unwrap();
        let result: String = conn.get("foo").await.unwrap();
        assert_eq!(result, "bar");
    }

    // build our application with a route
    let app = Router::new()
        .route("/api/hash", get(handler::get_hash))
        .route("/api/authorize", post(handler::authenticate))
        .route("/api/review", post(handler::new_review))
        .route("/api/shorten", post(handler::shorten_url))
        .route("/s/:short_url", get(handler::redirect_short))
        .with_state(pool);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}
