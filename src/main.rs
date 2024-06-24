mod toot;
mod redict;
mod auth;

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
use serde::{Serialize, Deserialize};
use serde_json::json;
use uuid::Uuid;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;
use redis::AsyncCommands;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

type ConnectionPool = Pool<RedisConnectionManager>;

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
        .route("/api/hash", get(get_hash))
        .route("/api/authorize", post(authenticate))
        .route("/api/review", post(new_review))
        .route("/api/shorten", post(shorten_url))
        .route("/s/:short_url", get(redirect_short))
        .with_state(pool);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
struct CreateReview {
    url: String,
    review: String,
    schedule: String,
}

async fn new_review(
    headers: HeaderMap,
    State(pool): State<ConnectionPool>,
    Json(input): Json<CreateReview>
) -> impl IntoResponse {
    let review = Review{
        id: Uuid::new_v4(),
        url: input.url,
        review: input.review,
        schedule: input.schedule,
        post_url: "".to_string(),
    };

    if let Err(e) = auth::verify_header(headers).await {
        tracing::warn!("auth error: {}", e);
        return (StatusCode::UNAUTHORIZED, Json(json!("Unauthorized")));
    }

    let conn = match pool.get().await.map_err(internal_error) {
        Ok(conn) => conn,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.1)));
        }
    };

    match redict::save_review(conn, &review).await {
        Ok(_) => { tracing::debug!("saved review {}", review.id.to_string()) },
        Err(e) => {
            tracing::error!("failed to save review {}: {}", review.id.to_string(), e.to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.to_string())));
        }
    }

    match toot::create_toot(review).await {
        Ok(res) => (StatusCode::CREATED, Json(res)),
        Err(e) => {
            eprintln!("error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!("Internal Server Error")))
        }
    }
}

#[derive(Debug, Serialize, Clone)]
struct Review {
    id: Uuid,
    url: String,
    review: String,
    schedule: String,
    post_url: String,
}

#[derive(Debug, Deserialize)]
struct CreateSession {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct SessionResponse {
    status: String,
    token: String,
}

async fn authenticate(
    State(pool): State<ConnectionPool>,
    Json(input): Json<CreateSession>
) -> impl IntoResponse {
    tracing::debug!("auth attempt from user {}", input.username);

    let conn = match pool.get().await.map_err(internal_error) {
        Ok(conn) => conn,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.1)));
        }
    };

    if let Err(err) = redict::try_auth(conn, &input).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, json!(err.to_string()).into());
    }

    let token = match auth::get_token().await {
        Ok(token) => token,
        Err(err) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, json!(err.to_string()).into());
        }
    };

    let session = SessionResponse{
        status: "OK".into(),
        token
    };

    (StatusCode::OK, Json(json!(session)))
}

async fn get_hash(
    Query(params): Query<HashMap<String, String>>
) -> &'static str {
    let plain_text = match params.get("plain") {
        Some(t) => t,
        None => return "Empty"
    };
    let hash = match auth::hash_str(plain_text).await {
        Ok(hash) => hash,
        Err(_) => {
            return "Not OK";
        }
    };

    tracing::debug!("hash: {}", hash);

    "OK"
}

#[derive(Deserialize, Debug)]
pub struct ShortenRequest {
    pub url: String,
    pub short: String,
}

pub async fn shorten_url(
    headers: HeaderMap,
    State(pool): State<ConnectionPool>,
    Json(request): Json<ShortenRequest>
) -> impl IntoResponse {
    if let Err(e) = auth::verify_header(headers).await {
        tracing::warn!("auth error: {}", e);
        return (StatusCode::UNAUTHORIZED, Json(json!("Unauthorized")));
    }

    let conn = match pool.get().await.map_err(internal_error) {
        Ok(conn) => conn,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.1)));
        }
    };

    if let Err(e) = redict::shorten_link(conn, &request).await {
        tracing::warn!("auth error: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!("Internal Server Error")));
    }

    let base_url = match std::env::var("OXP_BASE_URL") {
        Ok(url) => url,
        Err(e) => {
            tracing::warn!("OXP_BASE_URL: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!("Internal Server Error")));
        }
    };

    let short_url = format!("{}/s/{}", base_url.as_str(), request.short.as_str());

    (StatusCode::OK, Json(json!({"status": "ok", "short_url": short_url})))
}

pub async fn redirect_short(
    Path(short): Path<String>,
    State(pool): State<ConnectionPool>,
) -> Response {
    let conn = match pool.get().await.map_err(internal_error) {
        Ok(conn) => conn,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.1))).into_response();
        }
    };

    tracing::debug!(short);

    match redict::get_link(conn, short).await {
        Ok(long) => {
            Redirect::permanent(long.as_str()).into_response()
        },
        Err(e) => {
            tracing::warn!("{}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!("Internal Server Error"))).into_response()
        },
    }
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}