<!-- nav:start -->
**Tutorial 15 of 17** | [Index](00-index.md) | [Previous: 14 Login, register and sessions (`auth` module)](14-auth-and-sessions.md) | [Next: 16 The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)

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
**15. [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)**  
16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)  
17. [Media library (`media` module)](17-media-library.md)  

</details>
<!-- nav:end -->

# 15. Roles and permissions (`role-permission` module)

Ported from Dokani. Roles and permissions are defined **in code**, synced into the database, and checked per request.

## The hierarchy

Each role keeps every permission of the roles below it:

```
new_employee < business_development < marketing < employee < admin < developer
```

| Role | Adds on top of the previous role |
|---|---|
| New Employee | `dashboard.view` (the admin dashboard) |
| Business Development | `users.view`, `pages.view` |
| Marketing | `pages.manage`, `media.view` |
| Employee | `settings.view`, `logs.view`, `queue.view` |
| Admin | `dashboard.manage`, `users.manage`, `media.manage`, `settings.manage`, `logs.manage`, `queue.manage` |
| Developer | **everything** (`*`), including the role-permission manager: `roles.view`, `roles.assign`, `roles.sync` |

The role-permission manager belongs to the developer only. Admins cannot see or change roles.

## Where things are defined

`modules/role-permission/src/definitions.rs`:

- `AREAS`: each area gets `area.view` and `area.manage` automatically.
- `role_definitions()`: the six roles with a rank and the permissions each one **adds** (`"pages.*"` and `"*"` work as wildcards).

All permissions live in this one module, so you always know where to look. When you add a feature, add its area here. The developer role picks it up automatically.

## Commands

```sh
cargo modular command:run roles:sync                      # make the database match the code
cargo modular command:run roles:list
cargo modular command:run roles:assign email:demo@gmail.com role:developer
cargo modular command:run roles:revoke email:demo@gmail.com
```

`db seed` also syncs. The admin page `/admin/roles` shows the roles and a permission matrix, and developers can change someone's role from `/admin/users` (never their own, so nobody locks themselves out).

## Rules

- One role per person (`role_users.user_id` is unique), plus optional **direct permissions** for exceptions.
- Role-management permissions (`roles.*`) can only come from a role, never as a direct exception.
- Removing something from the code **deactivates** it on the next sync instead of deleting rows.
- Sync checks the definitions: ranks must go 1, 2, 3 ...; every pattern must match a real permission.

## Checking permissions

```rust
user.access.can("pages.manage")       // bool; wildcards in what was granted are honoured
user.access.require("users.view")?;   // Err(403) when missing
user.access.has_role("developer")
```

The "Admin dashboard" link in the header appears for anyone who `can("dashboard.view")`.

<!-- foot:start -->
---
[Previous: 14 Login, register and sessions (`auth` module)](14-auth-and-sessions.md) | [Index](00-index.md) | [Next: 16 The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)
<!-- foot:end -->
