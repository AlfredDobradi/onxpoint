use axum::{
    extract::{
        Json, Path, Query, State
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Redirect},
};
use std::collections::HashMap;
use serde_json::json;
use uuid::Uuid;

use crate::{
    auth, error, model, redict, toot
};

pub async fn get_hash(
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

pub async fn authenticate(
    State(pool): State<model::ConnectionPool>,
    Json(input): Json<model::CreateSession>
) -> impl IntoResponse {
    tracing::debug!("auth attempt from user {}", input.username);

    let conn = match pool.get().await.map_err(error::internal_error) {
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

    let session = model::SessionResponse{
        status: "OK".into(),
        token
    };

    (StatusCode::OK, Json(json!(session)))
}

pub async fn new_review(
    headers: HeaderMap,
    State(pool): State<model::ConnectionPool>,
    Json(input): Json<model::CreateReview>
) -> impl IntoResponse {
    let review = model::Review{
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

    let conn = match pool.get().await.map_err(error::internal_error) {
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

pub async fn shorten_url(
    headers: HeaderMap,
    State(pool): State<model::ConnectionPool>,
    Json(request): Json<model::ShortenRequest>
) -> impl IntoResponse {
    if let Err(e) = auth::verify_header(headers).await {
        tracing::warn!("auth error: {}", e);
        return (StatusCode::UNAUTHORIZED, Json(json!("Unauthorized")));
    }

    let conn = match pool.get().await.map_err(error::internal_error) {
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
    State(pool): State<model::ConnectionPool>,
) -> Response {
    let conn = match pool.get().await.map_err(error::internal_error) {
        Ok(conn) => conn,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(e.1))).into_response();
        }
    };

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
