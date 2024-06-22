mod toot;

use anyhow::Result;
use axum::{
    routing::post,
    extract::State,
    http::StatusCode,
    Json,
    Router,
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
        Err(e) => { panic!("{}", e); }
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
        .route("/api/review", post(new_review))
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

    let mut conn = match pool.get().await.map_err(internal_error) {
        Ok(conn) => conn,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.1)));
        }
    };

    let key = format!("reviews/{}", review.id.to_string());
    match conn.set::<&String, String, String>(&key, json!(review).to_string()).await {
        Ok(_) => { tracing::debug!("saved key {}", &key) },
        Err(e) => {
            tracing::error!("failed to save key: {}", e.to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.to_string())));
        }
    };

    match conn.sadd::<&str, &String, i32>("reviews", &key).await {
        Ok(_) => { tracing::debug!("added key {} to index", key) },
        Err(e) => {
            tracing::error!("failed to save key: {}", e.to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.to_string())));
        }
    };

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

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}