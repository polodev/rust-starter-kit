# rust-eloquent-tinker

A development console in the style of `php artisan tinker`, for a project that shares its database
with Laravel. It opens **PsySH** (the shell behind artisan tinker) with **Eloquent** models and the
**`DB` facade** pointed at this app's database, so you can read and update data in PHP while the app
itself stays Rust. It is a developer tool only and is not part of the running app.

## What you need

- PHP 8.2 or newer (with `pdo_mysql`) and Composer on your machine.
- Nothing else. The first run installs the PHP packages into `modules/rust-eloquent-tinker/php/vendor`
  (about a minute, once). The database address is read from the app's own config, so there is no
  second `.env` to keep in step.

## Use it

```sh
cargo run -- task tinker:eloquent                       # interactive shell (or: just eloquent)
cargo run -- task tinker:eloquent run:"User::count()"   # run one expression and print the result
```

Inside the shell it feels like artisan tinker:

```php
User::count();
User::where('email', 'demo@gmail.com')->first();
User::first()->roles->pluck('name');                    // relationships work
Page::with('translations')->get();
Contact::where('status', 'new')->count();
Media::where('collection_name', 'logo')->get();         // Spatie's media table

DB::table('users')->where('id', 1)->update(['name' => 'Ada']);
DB::select('select id, email from users limit 5');
DB::transaction(function () { /* ...all or nothing... */ });
DB::beginTransaction(); /* try things */ DB::rollBack();
```

Every model has a short global name (`User`, `Role`, `Permission`, `Page`, `PageTranslation`,
`Media`, `Contact`, `Session`, `PersonalAccessToken`, `SocialAccount`) and allows mass assignment.
Models print as their attributes. `now()` and `today()` are available.

The models are in `php/models/`. Add one by creating `php/models/Thing.php` (extend `BaseModel`, set
`$table`); it gets its global name automatically.

## Things to know

- **Passwords are Argon2id, not bcrypt.** The Rust app only accepts Argon2id hashes. `bcrypt()` or
  `Hash::make()` would create a password nobody can log in with. Assigning `password` on a `User`
  hashes it correctly for you; for a raw value use `rust_password('secret')`. (A password set this way
  signs in through the Rust login; this is tested.)
- **`users.pid` is 16 raw bytes** in this app. The `User` model fills it and `$user->public_id`
  shows it as text. `api_key` is filled too.
- **Media ids and paths:** the `media` table is Spatie's own. `$media->path` is `{id}/{file_name}` and
  `$media->url` is the address the Rust app serves.
- **You are changing the real database.** There is no confirmation. In production the command refuses
  to start unless you add `confirm:true`. Use `DB::beginTransaction()` and `DB::rollBack()` to try
  something safely.
- **No framework features:** this is Eloquent and the `DB` facade, not the whole Laravel framework.
  There is no `Cache`, `Auth`, `Mail`, `route()` or queue. Service-style helpers other than `now()`,
  `today()` and `collect()` are not loaded.
- The PHP packages (`php/vendor`, `php/composer.lock`) are git-ignored.

## Not what you need?

For a plain SQL prompt (no PHP), use the built-in console instead:

```sh
cargo run -- task tinker      # SQL console with .tables, .desc, .begin / .commit / .rollback
```
