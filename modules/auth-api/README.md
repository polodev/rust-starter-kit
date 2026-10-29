# auth-api

Laravel Sanctum-style token API. Every API in this app starts with `/api`; this one is `/api/v1/auth`.

| Method | Path | Token | Reply |
|---|---|---|---|
| POST | `/api/v1/auth/register` | no | 201 `{ "user", "token" }` |
| POST | `/api/v1/auth/login` | no | 200 `{ "user", "token" }` |
| GET | `/api/v1/auth/user` | yes | `{ "data": user }` |
| POST | `/api/v1/auth/logout` | yes | revokes this token |
| POST | `/api/v1/auth/logout-all` | yes | revokes all tokens |
| GET | `/api/v1/auth/tokens` | yes | `{ "data": [token] }` |
| DELETE | `/api/v1/auth/tokens/{id}` | yes | revokes one token |

Send the token as `Authorization: Bearer {id}|{secret}`. Only the SHA-256 of the secret is stored.
Errors are Laravel-shaped: `401 {"message":"Unauthenticated."}`, `422 {"message","errors":{field:[..]}}`,
`429` after 5 failed logins. `device_name` names the token. Expired tokens are pruned daily
(`auth-api:prune-tokens`).

## Protecting routes in another API module

```rust
use module_auth_api::ApiAuth;

async fn orders(ApiAuth(user): ApiAuth) -> Response { /* user.id, user.access.can("orders.view") */ }
```

Add `module-auth-api = { path = "../auth-api" }` to that module's `Cargo.toml`.
