//! `GET /contact` shows the page; `POST /contact` handles the form. When the request carries
//! `X-Component: 1` (the component script) only the form's HTML comes back and the script swaps it
//! in place; a plain browser post gets the whole page, so the form also works without JavaScript.

use std::time::Duration;

use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Form,
};
use chrono::Utc;
use loco_rs::prelude::*;
use module_auth::{same_origin, valid_email};
use module_site_layout::Site;
use sea_orm::{ActiveModelTrait, Set};
use serde::Deserialize;
use serde_json::{json, Value};

use super::wants_fragment;
use crate::entities::contacts;

const MAX_PER_WINDOW: u32 = 5;
const WINDOW: Duration = Duration::from_secs(10 * 60);

#[derive(Deserialize, Default)]
#[serde(default)]
struct ContactForm {
    name: String,
    email: String,
    subject: String,
    message: String,
    /// The honeypot. People never see this field, so only a bot fills it in.
    website: String,
}

fn client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|list| list.split(',').next())
        .map(|ip| ip.trim().chars().take(45).collect::<String>())
        .filter(|ip| !ip.is_empty())
}

fn page(
    v: &TeraView,
    site: &Site,
    headers: &HeaderMap,
    status: StatusCode,
    data: Value,
) -> Result<Response> {
    let template = if wants_fragment(headers) {
        "contact/_form.html"
    } else {
        "contact/page.html"
    };
    let body = v.render(template, site.data(data))?;
    Ok((status, Html(body)).into_response())
}

#[debug_handler]
async fn show(
    ViewEngine(v): ViewEngine<TeraView>,
    site: Site,
    headers: HeaderMap,
) -> Result<Response> {
    page(
        &v,
        &site,
        &headers,
        StatusCode::OK,
        json!({ "values": {}, "errors": {} }),
    )
}

#[debug_handler]
async fn submit(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    site: Site,
    headers: HeaderMap,
    Form(form): Form<ContactForm>,
) -> Result<Response> {
    let name = form.name.trim().to_string();
    let email = form.email.trim().to_lowercase();
    let subject = form.subject.trim().to_string();
    let message = form.message.trim().to_string();
    let values = json!({ "name": name, "email": email, "subject": subject, "message": message });
    let render = |status: StatusCode, extra: Value| {
        let mut data = json!({ "values": values, "errors": {} });
        if let (Some(base), Value::Object(extra)) = (data.as_object_mut(), extra) {
            base.extend(extra);
        }
        page(&v, &site, &headers, status, data)
    };

    if !same_origin(&headers) {
        return render(
            StatusCode::FORBIDDEN,
            json!({ "failure": "request-rejected" }),
        );
    }

    // Honeypot: answer exactly like a success so the bot learns nothing, and keep nothing.
    if !form.website.trim().is_empty() {
        return page(
            &v,
            &site,
            &headers,
            StatusCode::OK,
            json!({ "values": {}, "errors": {}, "sent": true }),
        );
    }

    let mut errors = serde_json::Map::new();
    if !(2..=80).contains(&name.chars().count()) {
        errors.insert("name".into(), json!("contact-error-name"));
    }
    if !valid_email(&email) {
        errors.insert("email".into(), json!("contact-error-email"));
    }
    if subject.chars().count() > 150 {
        errors.insert("subject".into(), json!("contact-error-subject"));
    }
    if !(10..=5000).contains(&message.chars().count()) {
        errors.insert("message".into(), json!("contact-error-message"));
    }
    if !errors.is_empty() {
        return render(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "errors": errors }),
        );
    }

    // A person sending a few messages is fine; a script sending dozens is not.
    let ip = client_ip(&headers);
    if let Some(ip) = &ip {
        let key = format!("contact-rate:{ip}");
        let sent = ctx.cache.get::<u32>(&key).await.ok().flatten().unwrap_or(0);
        if sent >= MAX_PER_WINDOW {
            return render(
                StatusCode::TOO_MANY_REQUESTS,
                json!({ "failure": "contact-error-throttled" }),
            );
        }
        let _ = ctx
            .cache
            .insert_with_expiry(&key, &(sent + 1), WINDOW)
            .await;
    }

    contacts::ActiveModel {
        name: Set(name),
        email: Set(email),
        subject: Set((!subject.is_empty()).then_some(subject)),
        message: Set(message),
        status: Set(contacts::STATUS_NEW.to_string()),
        locale: Set(site.lang().to_string()),
        ip_address: Set(ip),
        user_agent: Set(headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(|agent| agent.chars().take(255).collect())),
        created_at: Set(Utc::now().fixed_offset()),
        updated_at: Set(Utc::now().fixed_offset()),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await?;

    page(
        &v,
        &site,
        &headers,
        StatusCode::OK,
        json!({ "values": {}, "errors": {}, "sent": true }),
    )
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new().add("/contact", get(show).post(submit))]
}
