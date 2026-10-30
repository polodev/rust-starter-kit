# activity-log

An activity log in the style of Spatie laravel-activitylog, on Spatie's own `activity_log` table, so a Laravel
app and this app can share one log. Writing entries is done with the `loco-activity-log` package; this module
adds the admin screens and the clean command.

## Write an entry

```rust
use loco_activity_log::{activity, changes};

activity(&ctx.db)
    .in_log("users")                          // inLog('users'); the default log is "default"
    .caused_by("user", admin.id)              // causedBy($admin); leave out for a system action
    .performed_on("user", target.id)          // performedOn($target)
    .event("updated")
    .with_changes(changes(&old, &new, &[]))   // {"attributes": {...}, "old": {...}}: only what changed
    .with_property("ip", "10.0.0.1")          // withProperty
    .log_quietly("Updated the user's details")  // never fails the request; use .log() to get the Result
    .await;
```

- `:properties.key` in the description is replaced with that property, as in Spatie.
- `ACTIVITY_LOGGER_ENABLED=false` switches logging off.
- Subject and causer ids are stored as text, so numeric and uuid models both work. Use the same type names your
  Laravel app stores (a morph alias or class name); this app uses `user`, `page`, `contact`.
- Never put secrets (passwords, tokens) in properties.

## Already logged here

Sign in, sign out and registration (`auth`); name and password changes (`account`); user edits, generated
passwords and profile image changes (`users`); role and permission changes (`roles`); page text and
publish or hide (`pages`); contact replied status (`contacts`).

## Admin

`/admin/activity-log` (`activity_logs.view`): list with log, event and text filters, 50 per page; the ID opens an entry with
a before/after table of its changes. `activity_logs.manage` can delete entries and clean old ones.

## Command

`cargo run -- task activitylog:clean days:90 log:users` deletes entries older than N days (default 365,
optionally one log), like `php artisan activitylog:clean`. It runs daily at 03:20.
