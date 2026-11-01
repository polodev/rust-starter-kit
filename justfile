# Common commands. Install `just` or copy the lines you need.
css:
    ./tools/tailwindcss -c tailwind.config.js -i assets/css/input.css -o assets/static/css/app.css --minify

css-watch:
    ./tools/tailwindcss -c tailwind.config.js -i assets/css/input.css -o assets/static/css/app.css --watch

dev: css
    cargo run -- start --server-and-worker

migrate:
    cargo run -- db migrate

seed:
    cargo run -- db seed

test:
    cargo test --workspace

tinker:
    cargo run -- task tinker

eloquent:
    cargo run -- task tinker:eloquent
