use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;
use bb8::Pool;
use bb8_redis::RedisConnectionManager;

pub type ConnectionPool = Pool<RedisConnectionManager>;

#[derive(Debug, Deserialize)]
pub struct CreateReview {
    pub url: String,
    pub review: String,
    pub schedule: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct Review {
    pub id: Uuid,
    pub url: String,
    pub review: String,
    pub schedule: String,
    pub post_url: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSession {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub status: String,
    pub token: String,
}

#[derive(Deserialize, Debug)]
pub struct ShortenRequest {
    pub url: String,
    pub short: String,
}