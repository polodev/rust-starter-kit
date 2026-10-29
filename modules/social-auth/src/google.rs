//! Talking to Google: the sign-in URL, trading a code for the person, and checking an ID token.
//! Settings come from the environment (secrets stay out of the repository):
//! `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` and optionally `GOOGLE_REDIRECT_URL`.
//! The endpoint URLs can be overridden with `GOOGLE_*_URL` so tests can use a fake Google.

use std::{env, time::Duration};

use serde::Deserialize;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const TOKENINFO_URL: &str = "https://oauth2.googleapis.com/tokeninfo";

#[derive(Debug)]
pub enum GoogleError {
    NotConfigured,
    Rejected(&'static str),
    Unreachable,
}

/// The person Google vouches for.
#[derive(Debug, Clone)]
pub struct GoogleProfile {
    pub subject: String,
    pub email: String,
    pub name: Option<String>,
}

fn setting(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub fn client_id() -> Option<String> {
    env::var("GOOGLE_CLIENT_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Every client id whose ID tokens we accept: `GOOGLE_CLIENT_ID` plus the comma separated
/// `GOOGLE_CLIENT_IDS` (Android, iOS and web apps each have their own client id).
pub fn accepted_client_ids() -> Vec<String> {
    let extra = env::var("GOOGLE_CLIENT_IDS").unwrap_or_default();
    client_id()
        .into_iter()
        .chain(extra.split(',').map(|id| id.trim().to_string()))
        .filter(|id| !id.is_empty())
        .collect()
}

/// New Google users get an account unless `GOOGLE_AUTO_CREATE=false` (then only people who
/// already have an account, matched by email, can sign in).
pub fn auto_create() -> bool {
    !env::var("GOOGLE_AUTO_CREATE").is_ok_and(|value| matches!(value.trim(), "false" | "0" | "no"))
}

fn client_secret() -> Option<String> {
    env::var("GOOGLE_CLIENT_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

pub fn redirect_uri(site_url: &str) -> String {
    setting(
        "GOOGLE_REDIRECT_URL",
        &format!("{site_url}/auth/google/callback"),
    )
}

pub fn authorization_url(site_url: &str, state: &str) -> Result<String, GoogleError> {
    let client_id = client_id().ok_or(GoogleError::NotConfigured)?;
    let url = reqwest::Url::parse_with_params(
        &setting("GOOGLE_AUTH_URL", AUTH_URL),
        &[
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect_uri(site_url).as_str()),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("state", state),
            ("prompt", "select_account"),
        ],
    )
    .map_err(|_| GoogleError::NotConfigured)?;
    Ok(url.to_string())
}

fn client() -> Result<reqwest::Client, GoogleError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| GoogleError::Unreachable)
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    email: Option<String>,
    // Google sends a bool from userinfo and the string "true" from tokeninfo.
    email_verified: Option<serde_json::Value>,
    name: Option<String>,
    aud: Option<String>,
}

fn verified(claims: Claims) -> Result<GoogleProfile, GoogleError> {
    let email_verified = match &claims.email_verified {
        Some(serde_json::Value::Bool(flag)) => *flag,
        Some(serde_json::Value::String(text)) => text == "true",
        _ => false,
    };
    let email = claims
        .email
        .map(|email| email.trim().to_lowercase())
        .filter(|email| !email.is_empty());
    match (email, email_verified) {
        (Some(email), true) => Ok(GoogleProfile {
            subject: claims.sub,
            email,
            name: claims.name.filter(|name| !name.trim().is_empty()),
        }),
        _ => Err(GoogleError::Rejected("email-not-verified")),
    }
}

/// Browser flow: trade the `code` from the callback for the person.
pub async fn profile_for_code(site_url: &str, code: &str) -> Result<GoogleProfile, GoogleError> {
    let (id, secret) = client_id()
        .zip(client_secret())
        .ok_or(GoogleError::NotConfigured)?;
    let http = client()?;
    let token = http
        .post(setting("GOOGLE_TOKEN_URL", TOKEN_URL))
        .form(&[
            ("code", code),
            ("client_id", id.as_str()),
            ("client_secret", secret.as_str()),
            ("redirect_uri", redirect_uri(site_url).as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|_| GoogleError::Unreachable)?;
    if !token.status().is_success() {
        return Err(GoogleError::Rejected("code-rejected"));
    }
    let token: TokenReply = token
        .json()
        .await
        .map_err(|_| GoogleError::Rejected("bad-token-reply"))?;
    let info = http
        .get(setting("GOOGLE_USERINFO_URL", USERINFO_URL))
        .bearer_auth(token.access_token)
        .send()
        .await
        .map_err(|_| GoogleError::Unreachable)?;
    if !info.status().is_success() {
        return Err(GoogleError::Rejected("userinfo-rejected"));
    }
    verified(
        info.json()
            .await
            .map_err(|_| GoogleError::Rejected("bad-userinfo"))?,
    )
}

/// API flow: a mobile or single page app signs in with Google itself and sends the ID token.
/// Google checks the signature and expiry; we check it was issued for our client id.
pub async fn profile_for_id_token(id_token: &str) -> Result<GoogleProfile, GoogleError> {
    let accepted = accepted_client_ids();
    if accepted.is_empty() {
        return Err(GoogleError::NotConfigured);
    }
    let reply = client()?
        .get(setting("GOOGLE_TOKENINFO_URL", TOKENINFO_URL))
        .query(&[("id_token", id_token)])
        .send()
        .await
        .map_err(|_| GoogleError::Unreachable)?;
    if !reply.status().is_success() {
        return Err(GoogleError::Rejected("token-rejected"));
    }
    let claims: Claims = reply
        .json()
        .await
        .map_err(|_| GoogleError::Rejected("bad-token-reply"))?;
    if !claims
        .aud
        .as_ref()
        .is_some_and(|aud| accepted.contains(aud))
    {
        return Err(GoogleError::Rejected("wrong-audience"));
    }
    verified(claims)
}
