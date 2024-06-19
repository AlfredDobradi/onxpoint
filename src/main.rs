mod toot;

use axum::{
    routing::{get, post},
    http::StatusCode,
    Json,
    Router,
};
use axum::response::IntoResponse;
use serde::{Serialize, Deserialize};
use serde_json::json;
use uuid::Uuid;
use crate::toot::create_toot;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        .route("/api/review", post(new_review));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

#[derive(Debug, Deserialize)]
struct CreateReview {
    url: String,
    review: String,
    schedule: String,
}

async fn new_review(
    Json(input): Json<CreateReview>
) -> impl IntoResponse {
    let review = Review{
        id: Uuid::new_v4(),
        url: input.url,
        review: input.review,
        schedule: input.schedule,
        post_url: "".to_string(),
    };

    tracing::info!("new review: {:?}", review);

    match create_toot(review).await {
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