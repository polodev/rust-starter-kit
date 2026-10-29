# 14. Login, register and sessions (`auth` module)

The `auth` module owns everything about who you are: login, register, logout, the session cookie and the middleware that reads it.

## Pages

| URL | What it does |
|---|---|
| `GET/POST /register` | Create an account. Sign-up signs you in. New accounts have **no role**. |
| `GET/POST /login` | Sign in. Accepts `?next=/some/page` and returns there afterwards. |
| `POST /logout` | Ends this session and clears the cookie. |

The header follows Dokani: guests see **Log in** (amber link) and **Register** (button); signed-in people see an avatar with a menu (Admin dashboard if they hold `dashboard.view`, My account, Log out).

## How a session works

1. Login creates a random 256-bit token and sets it as the `session` cookie (`HttpOnly`, `SameSite=Lax`, `Secure` in production, 30 days).
2. The database stores only the **SHA-256 of the token** in the `sessions` table, with the user, expiry and last use. A leaked table cannot be replayed as a login.
3. On every request a middleware (`modules/auth/src/middleware.rs`) reads the cookie, finds the live session, loads the person and their permissions, and puts them in the request. Using the site slides the expiry forward (written at most once an hour).
4. `auth:prune-sessions` deletes expired rows. It is scheduled hourly.

## Using it in your own handler

```rust
use module_auth::{Auth, OptionalAuth};

async fn secret(Auth(user): Auth) -> Result<Response> {        // not signed in? redirect to /login?next=...
    user.access.require("pages.manage")?;                       // 403 when missing
    ...
}

async fn maybe(OptionalAuth(user): OptionalAuth) { ... }       // user is Option<AuthUser>
```

`AuthUser` carries `id`, `name`, `email`, `session_id` and `access` (role and permissions). The shared page data (`Site`) also carries the person for the header.

## Safety built in

- **CSRF:** every form post must come from this site. `same_origin()` checks the `Origin` (or `Referer`) host against `Host`. A post with neither header, or from another site, gets 403. Together with `SameSite=Lax` this stops other sites posting as you.
- **Open redirects:** `next` must be a local path. `//host`, full URLs, backslashes and `/login`/`/register` fall back to `/`.
- **Brute force:** five failed logins for an email lock it for 15 minutes (cache-based), even for the right password.
- **No account guessing:** a wrong password and an unknown email give the same message, and an unknown email still pays the cost of one password hash.
- **Passwords:** hashed with Argon2 (Loco). At least 8 characters.

## Good to know

- No mailer is wired yet, so register marks the email as verified. When mail is added, switch to a verification link.
- The login lockout uses the in-memory cache, so with several servers use Redis so they share the counter.
- The demo account (`demo@gmail.com` / `secret123$`) exists in **development only**. It is created by `cargo run -- db seed` and is a developer.

Next: [15-roles-and-permissions.md](15-roles-and-permissions.md)
