# 09. Cache and sessions

The kit uses two kinds of "remembering".

| | Short-lived | Long-lived |
|---|---|---|
| Purpose | Remember a database result | Remember a person |
| Laravel idea | `Cache::remember` | Session and "remember me" |
| Lifetime | Seconds to minutes | Days to a year |
| Stored | Server memory (Redis later) | Browser cookie, plus database for login |
| Built | Yes | Language cookie yes, login not yet |

## Short-lived: `remember`

```rust
use loco_modular::remember;

let page = remember(&ctx, &format!("pages:{slug}:{code}"), Duration::from_secs(300), async {
    find_page(&ctx.db, &slug, code).await
})
.await?;
```

It returns the cached value if present. Otherwise it runs the query, stores the result for the given time, and returns it.

- The value must be `Serialize + DeserializeOwned`.
- Build keys from everything that changes the result (`slug` and language here).
- If the cache itself fails, `remember` runs the query directly, so a cache outage never breaks a page.
- Cache what is cheap to be slightly stale: public pages, lookups, counts. Do not cache per-user secrets under a shared key.
- When you edit content in an admin, delete the matching key (`ctx.cache.remove(key)`) or wait for the time to pass.

Configuration in `config/*.yaml`:

```yaml
cache:
  kind: InMem
  max_capacity: 1000
```

For several servers, switch to Redis (`kind: Redis`) so they share one cache.

## Long-lived: cookies

The language choice is a cookie that lasts a year:

```
lang=bn; Path=/; Max-Age=31536000; SameSite=Lax
```

That is fine for a preference. A **login** needs more care, and it is the next piece to build:

- a random session id in a cookie marked `HttpOnly`, `Secure` and `SameSite=Lax`
- the session stored in the database (user id, expiry, last use), so it can be revoked
- a refresh on use, so active users stay signed in
- the language preference saved on the user record, so it follows them across devices

Never put the user id or any secret in a plain cookie, and never trust cookie values without checking them, as `normalize()` does for the language.

Next: [10-testing.md](10-testing.md)
