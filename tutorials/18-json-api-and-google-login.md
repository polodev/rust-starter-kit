<!-- nav:start -->
**Tutorial 18 of 21** | [Index](00-index.md) | [Previous: 17 Media library (`media` module)](17-media-library.md) | [Next: 19 Components and the contact form](19-components-and-contact-form.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
02. [Setup](02-setup.md)  
03. [Rust essentials](03-rust-essentials.md)  
04. [Project structure](04-project-structure.md)  
05. [Modules and `cargo modular`](05-modules.md)  
06. [Routes, controllers and views](06-routes-controllers-views.md)  
07. [Database: migrations, entities, queries, seeding](07-database.md)  
08. [A bilingual site (English and Bangla)](08-bilingual-site.md)  
09. [Cache and sessions](09-cache-and-sessions.md)  
10. [Testing](10-testing.md)  
11. [Assets and styling](11-assets-and-styling.md)  
12. [Cheat sheet and what to build next](12-cheatsheet.md)  
13. [Console commands and the scheduler](13-console-and-schedule.md)  
14. [Login, register and sessions (`auth` module)](14-auth-and-sessions.md)  
15. [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)  
16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)  
17. [Media library (`media` module)](17-media-library.md)  
**18. [JSON API and Google login](18-json-api-and-google-login.md)**  
19. [Components and the contact form](19-components-and-contact-form.md)  
20. [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)  
21. [Conventions](21-conventions.md)  

</details>
<!-- nav:end -->

# 18. JSON API and Google login (`auth-api`, `social-auth`)

Laravel Sanctum's idea, in two modules. Every API route starts with `/api`; ours are under `/api/v1`.

## Tokens (`auth-api`)

| Method | Path | Needs token | Reply |
|---|---|---|---|
| POST | `/api/v1/auth/register` | no | 201 `{ "user", "token" }` |
| POST | `/api/v1/auth/login` | no | 200 `{ "user", "token" }` |
| GET | `/api/v1/auth/user` | yes | `{ "data": user }` |
| POST | `/api/v1/auth/logout` | yes | revokes this token |
| POST | `/api/v1/auth/logout-all` | yes | revokes every token |
| GET / DELETE | `/api/v1/auth/tokens`, `/tokens/{id}` | yes | list, revoke one |

```sh
curl -s localhost:5150/api/v1/auth/login -H 'content-type: application/json' \
  -d '{"email":"demo@gmail.com","password":"secret123$","device_name":"phone"}'
# {"user":{...},"token":"7|9f2c..."}
curl -s localhost:5150/api/v1/auth/user -H 'authorization: Bearer 7|9f2c...'
```

- **Token:** `{id}|{secret}`; only the SHA-256 of the secret is stored. `device_name` names it. Expired tokens are pruned daily (`auth-api:prune-tokens`).
- **Errors in Laravel's shape:** `401 {"message":"Unauthenticated."}`, `422 {"message":"...","errors":{"email":["..."]}}`, `429` after 5 failed logins. Unknown email and wrong password answer the same.

## Protect your own endpoint

```rust
use module_auth_api::ApiAuth;

async fn orders(ApiAuth(user): ApiAuth) -> Response {
    // user.id, user.access.can("orders.view")
}
```

Add `module-auth-api = { path = "../auth-api" }` to the module's `Cargo.toml`.

## Sign in with Google (`social-auth`)

| Method | Path | What |
|---|---|---|
| GET | `/auth/google/redirect` | Website: sends the person to Google (with a random `state` cookie) |
| GET | `/auth/google/callback` | Website: checks `state`, signs in, sets the session cookie |
| POST | `/api/v1/auth/google` | API: `{ "id_token" }` (or `access_token`) returns `{ "user", "token" }` |

Set `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET` (add `{APP_URL}/auth/google/callback` as a redirect URI in Google Cloud). Without a client id the button is hidden.

- **Mobile and single-page apps:** sign in with Google's SDK and send the ID token. Its audience must be `GOOGLE_CLIENT_ID` or one of `GOOGLE_CLIENT_IDS` (comma separated: Android, iOS, web).
- **People:** a linked account signs in; otherwise an account with the same verified email is linked; otherwise a new verified account is created. `GOOGLE_AUTO_CREATE=false` allows only existing accounts.
- **Envelope:** the API reply also carries `data` and `message`, the shape many existing mobile clients already read.

Details: `modules/auth-api/README.md`, `modules/social-auth/README.md`.

<!-- foot:start -->
---
[Previous: 17 Media library (`media` module)](17-media-library.md) | [Index](00-index.md) | [Next: 19 Components and the contact form](19-components-and-contact-form.md)
<!-- foot:end -->
