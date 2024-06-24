use axum::http::HeaderMap;
use rusty_paseto::prelude::*;
use anyhow::Result;

async fn crypt_key() -> Result<PasetoSymmetricKey<V4, Local>> {
    let crypt_key = std::env::var("OXP_CRYPT_KEY")?;

    Ok(PasetoSymmetricKey::<V4, Local>::from(Key::from(crypt_key.as_bytes())))
}

pub async fn hash_str(
    plain_text: &String
) -> Result<String> {
    let hash = bcrypt::hash(&plain_text, bcrypt::DEFAULT_COST)?;
    Ok(hash)
}

pub async fn get_token() -> Result<String> {
    let key = crypt_key().await?;

    let token = PasetoBuilder::<V4, Local>::default()
    .build(&key)?;

    Ok(token)
}

async fn verify_token(token: &String) -> Result<()> {
    let key = crypt_key().await?;

    PasetoParser::<V4, Local>::default().parse(token.as_str(), &key)?;

    Ok(())
}

pub async fn verify_header(
    headers: HeaderMap
) -> Result<()> {
    tracing::debug!("headers: {:?}", &headers);

    let auth_header = match headers.get("Authorization") {
        Some(auth) => auth,
        None => {
            tracing::warn!("no authorization header");
            return Err(anyhow::format_err!("missing auth header"))
        }
    };

    if let Ok(auth_header_str) = auth_header.to_str() {
        let token = auth_header_str.strip_prefix("Bearer ").unwrap_or(auth_header_str).to_string();
        if let Err(_) = verify_token(&token).await {
            tracing::warn!("invalid authorization token");
            return Err(anyhow::format_err!("failed to verify token"));
        }
    };

    Ok(())
}