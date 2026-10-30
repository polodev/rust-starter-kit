<!-- nav:start -->
**Tutorial 2 of 21** | [Index](00-index.md) | [Previous: 1 Introduction](01-introduction.md) | [Next: 3 Rust essentials](03-rust-essentials.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
**02. [Setup](02-setup.md)**  
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
18. [JSON API and Google login](18-json-api-and-google-login.md)  
19. [Components and the contact form](19-components-and-contact-form.md)  
20. [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)  
21. [Conventions](21-conventions.md)  

</details>
<!-- nav:end -->

# 02. Setup

## Install the tools

```sh
# Rust (rustup installs cargo, the build tool and package manager)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustc --version && cargo --version

# MariaDB must be running on 127.0.0.1:3306 (Herd, Homebrew, or Docker all work)
mysql -uroot -e "select version()"
```

Optional: `cargo install just` for the shortcuts in the `justfile`.

## Create the databases

```sh
mysql -uroot -e "CREATE DATABASE rust_starter CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
mysql -uroot -e "CREATE DATABASE rust_starter_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
```

`utf8mb4` is required so Bangla text and emoji store correctly.

## Configure

```sh
cp .env.example .env
```

`.env` holds the database URLs and the R2 storage keys. It is ignored by git. Never commit it.

| Variable | Used for |
|---|---|
| `DATABASE_URL` | Development database (`rust_starter`) |
| `TEST_DATABASE_URL` | Test database. The name **must** end in `_test` |
| `R2_*` | Cloudflare R2 storage |

## Get Tailwind

Download the standalone Tailwind CLI into `tools/` (it is git-ignored):

```sh
mkdir -p tools
curl -sL -o tools/tailwindcss \
  https://github.com/tailwindlabs/tailwindcss/releases/download/v3.4.17/tailwindcss-macos-arm64
chmod +x tools/tailwindcss
```

Use `tailwindcss-macos-x64` or `tailwindcss-linux-x64` on other machines.

## Run it

```sh
./tools/tailwindcss -c tailwind.config.js -i assets/css/input.css -o assets/static/css/app.css --minify
cargo run -- db migrate
cargo run -- db seed
cargo run -- start
```

Open <http://localhost:5150>. The first `cargo` run compiles many crates and is slow. Later runs are fast.

Try these URLs:

- `/` home
- `/pages/about`, `/pages/privacy-policy`, `/pages/terms-of-service`
- `/pages/about?lang=bn` the same page in Bangla

## When something fails

| Symptom | Fix |
|---|---|
| `Connection refused` | MariaDB is not running on port 3306 |
| `Unknown database` | Create the database (see above) |
| Page has no styling | Build the CSS file (see Tailwind step) |
| `refusing to boot the test environment` | `TEST_DATABASE_URL` does not end in `_test`. This is the safety guard working |

<!-- foot:start -->
---
[Previous: 1 Introduction](01-introduction.md) | [Index](00-index.md) | [Next: 3 Rust essentials](03-rust-essentials.md)
<!-- foot:end -->
