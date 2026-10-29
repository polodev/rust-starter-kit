# 08. A bilingual site (English and Bangla)

Two kinds of text, handled two ways:

| Kind | Example | Where it lives |
|---|---|---|
| Interface strings | nav labels, buttons, footer | Fluent files in `assets/i18n/` |
| Content | About, Privacy Policy, Terms | Database table `page_translations` |

## Which language is shown?

`Locale` (in `modules/site-layout/src/locale.rs`) is an Axum extractor. It decides in this order:

1. `?lang=bn` in the URL
2. the `lang` cookie
3. the `Accept-Language` header, honouring quality values
4. English

Any handler can take `locale: Locale` and call `locale.code()` (`"en"` or `"bn"`).

## Switching language

`GET /language/bn?next=/pages/about` sets the cookie and redirects back:

```
HTTP/1.1 303 See Other
set-cookie: lang=bn; Path=/; Max-Age=31536000; SameSite=Lax
location: /pages/about
```

The `next` value must start with a single `/`. Anything else (`//evil.example`, `https://...`) is ignored and the user goes to `/`. This prevents an open redirect.

## Interface strings (Fluent)

`assets/i18n/en-US/main.ftl`:

```
nav-about = About
home-title = Welcome to your new website
```

`assets/i18n/bn-BD/main.ftl`:

```
nav-about = আমাদের সম্পর্কে
home-title = আপনার নতুন ওয়েবসাইটে স্বাগতম
```

In a template: `{{ t(key="nav-about", lang=tag) }}`. The handler passes `tag` as `en-US` or `bn-BD` using `fluent_tag(code)`.

Rules:

- Add every key to **both** files. A test (`every_translation_key_exists_in_both_languages`) fails if they differ.
- Put shared terms starting with `-` in `assets/shared.ftl`, outside `assets/i18n/`.

## Page content

Each page has one row in `pages` and one row per language in `page_translations`:

```
pages:              id=1  slug=about
page_translations:  page_id=1  locale=en  title=About us
                    page_id=1  locale=bn  title=আমাদের সম্পর্কে
```

The controller asks for the visitor's language first and falls back to English if that translation is missing, so a half-translated page never breaks.

To add a language later (for example Hindi): add `hi` to `SUPPORTED_LOCALES`, create `assets/i18n/hi-IN/main.ftl`, map it in `fluent_tag`, and add translations.

## Bangla typography

The layout loads **Noto Sans Bengali** and switches the body font class when `lang == "bn"`. Always test pages in Bangla: text is wider and may need different spacing.

## Try it

```sh
curl -s "http://localhost:5150/pages/about?lang=bn" | grep "<h1"
curl -s -H "Accept-Language: bn-BD" http://localhost:5150/pages/about | grep "<h1"
curl -s -i "http://localhost:5150/language/bn?next=/pages/about"
```

Next: [09-cache-and-sessions.md](09-cache-and-sessions.md)
