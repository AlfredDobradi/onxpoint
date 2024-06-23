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