# social-auth

Sign in with Google (the Laravel Socialite idea) for the website and the API. Google accounts are
linked to local users in `social_accounts`; an existing account with the same Google-verified email
is linked, otherwise a new verified account is created.

## Setup

Create an OAuth client in the Google Cloud console, add `{APP_URL}/auth/google/callback` as an
authorised redirect URI, then set:

```
GOOGLE_CLIENT_ID=...apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=...
# other apps' client ids whose ID tokens are accepted by the API (Android, iOS, web), comma separated
GOOGLE_CLIENT_IDS=ios-id.apps.googleusercontent.com,android-id.apps.googleusercontent.com
# false = only people who already have an account (matched by email) can sign in
GOOGLE_AUTO_CREATE=true
# optional, defaults to {server.host}:{port}/auth/google/callback
GOOGLE_REDIRECT_URL=https://example.com/auth/google/callback
```

Without `GOOGLE_CLIENT_ID` the login page hides the button and the routes answer "not set up".

## Routes

| Method | Path | What |
|---|---|---|
| GET | `/auth/google/redirect` | Website: sends the person to Google (random `state` cookie). |
| GET | `/auth/google/callback` | Website: checks `state`, signs in, sets the session cookie. |
| POST | `/api/v1/auth/google` | API: `{ "id_token", "device_name" }` returns `{ "user", "token" }`. |

For the API, the mobile app or single page app signs in with Google itself (Google Sign-In SDK) and
sends the ID token (`id_token`, or `access_token` as an alias); its audience must be `GOOGLE_CLIENT_ID` or one of `GOOGLE_CLIENT_IDS`. The reply also carries `data` and `message` in the shape those apps read. The token is checked by
Google's `tokeninfo` endpoint. The bearer token works exactly like one from `auth-api`.
