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

Next: [16-admin-dashboard.md](16-admin-dashboard.md)
