//! PDF generation, the way Spatie's Browsershot does it: render HTML (and CSS) in headless Chrome and print
//! it to PDF. Anything you can design in a web page can be a PDF, and Bangla text is shaped by the browser.
//!
//! ```ignore
//! let html = module_pdf::render_template(include_str!("../templates/pdf/invoice.html"), &data)?;
//! let bytes = module_pdf::html_to_pdf(&html, &PdfOptions::from_env()).await?;
//! ```
//!
//! Chrome or Chromium must be installed on the machine. It is found through `CHROME_PATH`, then the usual
//! install locations. Set `PDF_CHROME_NO_SANDBOX=true` when running as root in a container. Page size and
//! margins come from the page's own CSS (`@page { size: A4; margin: 18mm }`).

mod commands;
pub mod invoice;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use async_trait::async_trait;
use loco_modular::Module;
use loco_rs::{controller::Routes, task::Tasks, Error, Result};

pub use invoice::{Invoice, Line, Party};

/// How to run Chrome.
#[derive(Debug, Clone)]
pub struct PdfOptions {
    /// Path of Chrome or Chromium; found automatically when `None`.
    pub chrome_path: Option<String>,
    /// Chrome is stopped if it takes longer than this.
    pub timeout: Duration,
    /// Needed when Chrome runs as root, for example inside a container.
    pub no_sandbox: bool,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            chrome_path: None,
            timeout: Duration::from_secs(45),
            no_sandbox: false,
        }
    }
}

impl PdfOptions {
    /// Reads `CHROME_PATH` and `PDF_CHROME_NO_SANDBOX` from the environment.
    pub fn from_env() -> Self {
        Self {
            chrome_path: std::env::var("CHROME_PATH")
                .ok()
                .filter(|path| !path.trim().is_empty()),
            no_sandbox: std::env::var("PDF_CHROME_NO_SANDBOX").is_ok_and(|value| {
                matches!(value.trim().to_lowercase().as_str(), "true" | "1" | "yes")
            }),
            ..Self::default()
        }
    }
}

const CANDIDATES: [&str; 7] = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/snap/bin/chromium",
];

/// The Chrome or Chromium to use: the configured path, else the first one that exists.
pub fn find_chrome(configured: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = configured {
        return if Path::new(path).exists() {
            Ok(PathBuf::from(path))
        } else {
            Err(Error::string(&format!(
                "CHROME_PATH `{path}` does not exist"
            )))
        };
    }
    CANDIDATES
        .iter()
        .map(Path::new)
        .find(|path| path.exists())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            Error::string("could not find Chrome or Chromium; install it or set CHROME_PATH")
        })
}

/// Renders a Tera template from source text. HTML in the data is escaped.
pub fn render_template(source: &str, data: &serde_json::Value) -> Result<String> {
    let context = tera::Context::from_value(data.clone())
        .map_err(|error| Error::string(&format!("bad template data: {error}")))?;
    tera::Tera::one_off(source, &context, true)
        .map_err(|error| Error::string(&format!("could not render the template: {error:?}")))
}

/// Prints an HTML document to PDF with headless Chrome and returns the bytes.
pub async fn html_to_pdf(html: &str, options: &PdfOptions) -> Result<Vec<u8>> {
    let chrome = find_chrome(options.chrome_path.as_deref())?;
    let work = tempfile::tempdir().map_err(|error| Error::string(&error.to_string()))?;
    let page = work.path().join("page.html");
    let output = work.path().join("out.pdf");
    tokio::fs::write(&page, html)
        .await
        .map_err(|error| Error::string(&error.to_string()))?;

    let mut command = tokio::process::Command::new(&chrome);
    command
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--hide-scrollbars")
        .arg("--no-pdf-header-footer")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        // A private profile, so a Chrome that is already open does not swallow the request.
        .arg(format!(
            "--user-data-dir={}",
            work.path().join("profile").display()
        ))
        .arg(format!("--print-to-pdf={}", output.display()));
    if options.no_sandbox {
        command.arg("--no-sandbox");
    }
    command
        .arg(format!("file://{}", page.display()))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let child = command
        .spawn()
        .map_err(|error| Error::string(&format!("could not start Chrome: {error}")))?;
    let finished = tokio::time::timeout(options.timeout, child.wait_with_output())
        .await
        .map_err(|_| Error::string("Chrome took too long to print the page"))?
        .map_err(|error| Error::string(&error.to_string()))?;

    let bytes = tokio::fs::read(&output).await.unwrap_or_default();
    if !bytes.starts_with(b"%PDF-") {
        let reason: String = String::from_utf8_lossy(&finished.stderr)
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .unwrap_or("no output")
            .chars()
            .take(300)
            .collect();
        return Err(Error::string(&format!(
            "Chrome did not produce a PDF ({}): {reason}",
            finished.status
        )));
    }
    Ok(bytes)
}

pub struct PdfModule;

#[async_trait]
impl Module for PdfModule {
    fn name(&self) -> &'static str {
        "pdf"
    }

    fn routes(&self) -> Vec<Routes> {
        Vec::new()
    }

    fn register_tasks(&self, tasks: &mut Tasks) {
        commands::register(tasks);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn templates_render_and_escape_html() {
        let html =
            render_template("<p>{{ name }}</p>", &json!({"name": "<b>Ada & co</b>"})).unwrap();
        assert_eq!(html, "<p>&lt;b&gt;Ada &amp; co&lt;&#x2F;b&gt;</p>");
        assert!(render_template("{{ missing }}", &json!({})).is_err());
    }

    #[test]
    fn a_missing_chrome_is_explained() {
        let error = find_chrome(Some("/no/such/chrome"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("does not exist"));
    }
}
