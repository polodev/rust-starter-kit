# 11. Assets and styling

## Static files

Everything in `assets/static/` is served under `/static`. For example `assets/static/css/app.css` is `/static/css/app.css`. The layout links it.

## Tailwind without Node

The standalone Tailwind CLI lives in `tools/tailwindcss` (git-ignored).

| File | Role |
|---|---|
| `assets/css/input.css` | Your source: Tailwind layers plus custom components |
| `tailwind.config.js` | Which files to scan for class names |
| `assets/static/css/app.css` | Generated output; do not edit |

Build:

```sh
./tools/tailwindcss -c tailwind.config.js -i assets/css/input.css -o assets/static/css/app.css --minify
```

While designing, use `--watch` instead of `--minify` in a second terminal.

Tailwind only includes classes it finds in the scanned files. The config scans `modules/*/templates/**/*.html`, so a class used in a module template is picked up after a rebuild. If a new class has no effect, rebuild the CSS.

## Reusable styles

Repeated groups of classes belong in `input.css` under `@layer components`:

```css
@layer components {
  .page-body h2 { @apply mt-8 text-xl font-semibold text-slate-900; }
}
```

The `page-body` class styles the staff-written HTML of a page without adding classes inside the database content.

## Fonts

`layouts/site.html` loads Inter and Noto Sans Bengali from Google Fonts and sets `font-latin` or `font-bangla` on `<body>` by language. For a site with strict privacy needs, download the fonts and serve them from `assets/static/fonts/`.

## Changing the look

Edit `modules/site-layout/templates/layouts/site.html`. Every page extends it, so the header, footer and fonts change everywhere at once.

Next: [12-cheatsheet.md](12-cheatsheet.md)
