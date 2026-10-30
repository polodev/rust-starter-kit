# pdf

PDF generation the way Spatie's Browsershot does it: render HTML and CSS in **headless Chrome** and print it to PDF.
Anything you can design as a web page can be a PDF, and Bangla text is shaped correctly because the browser does it.

```rust
let html = module_pdf::render_template(include_str!("../templates/pdf/invoice.html"), &data)?; // Tera
let pdf: Vec<u8> = module_pdf::html_to_pdf(&html, &PdfOptions::from_env()).await?;
```

- **Needs** Chrome or Chromium on the machine. It is found through `CHROME_PATH`, then the usual install locations
  (macOS app, `/usr/bin/google-chrome`, `/usr/bin/chromium`, ...). In a container running as root set
  `PDF_CHROME_NO_SANDBOX=true`.
- **Page setup** is plain CSS: `@page { size: A4; margin: 18mm 16mm }`. Header and footer come from your own HTML.
- Chrome runs with a private temporary profile, so a Chrome you already have open is not disturbed. It is stopped
  after `timeout` (45 s by default).
- **Fonts:** the page's `font-family` decides. The sample uses `"Noto Sans Bengali", "Siyam Rupali"` first, so install
  a Bangla font on the server (for example `fonts-noto-core` on Debian or Ubuntu).

## Sample invoice

`module_pdf::Invoice` is a made-up invoice with exact money maths (minor units, tax rounded half up) and a Tera
template with Bangla text. Try it:

```sh
cargo run -- task pdf:sample out:/tmp/invoice.pdf
```

Use it as a pattern: copy `templates/pdf/invoice.html`, change the data, call `html_to_pdf`.

## Not included

Header and footer templates on every page, page numbers (use CSS counters), merging PDFs, and a render queue.
