use bytes::Bytes;

/// A file received from a visitor, before it is validated and stored.
#[derive(Debug, Clone)]
pub struct Upload {
    pub file_name: String,
    pub bytes: Bytes,
}

impl Upload {
    pub fn new(file_name: impl Into<String>, bytes: impl Into<Bytes>) -> Self {
        Self {
            file_name: file_name.into(),
            bytes: bytes.into(),
        }
    }

    /// The real content type, read from the first bytes. The client-supplied name and
    /// `Content-Type` are never trusted. SVG is not recognised on purpose (it can carry scripts).
    pub fn sniff_mime(&self) -> Option<&'static str> {
        let b = &self.bytes;
        if b.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some("image/png")
        } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some("image/jpeg")
        } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
            Some("image/gif")
        } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
            Some("image/webp")
        } else if b.starts_with(b"%PDF-") {
            Some("application/pdf")
        } else {
            None
        }
    }
}

pub fn extension_for(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

/// A safe base name: letters, digits, `-` and `_`, at most 80 characters, never empty.
pub fn sanitize_stem(file_name: &str) -> String {
    let stem = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = stem.rsplit_once('.').map_or(stem, |(name, _)| name);
    let mut cleaned = String::new();
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            cleaned.push(ch);
        } else if !cleaned.ends_with('-') {
            cleaned.push('-');
        }
    }
    let cleaned: String = cleaned.trim_matches('-').chars().take(80).collect();
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn content_decides_the_type_not_the_name() {
        assert_eq!(
            Upload::new("evil.png", &b"<?php echo 1;"[..]).sniff_mime(),
            None
        );
        assert_eq!(
            Upload::new("photo.exe", PNG).sniff_mime(),
            Some("image/png")
        );
        assert_eq!(
            Upload::new("a", &[0xFF, 0xD8, 0xFF, 0xE0][..]).sniff_mime(),
            Some("image/jpeg")
        );
        assert_eq!(
            Upload::new("a", &b"GIF89a...."[..]).sniff_mime(),
            Some("image/gif")
        );
        assert_eq!(
            Upload::new("a", &b"RIFF\0\0\0\0WEBPVP8 "[..]).sniff_mime(),
            Some("image/webp")
        );
        assert_eq!(
            Upload::new("a", &b"%PDF-1.7"[..]).sniff_mime(),
            Some("application/pdf")
        );
        assert_eq!(
            Upload::new("a.svg", &b"<svg onload=alert(1)>"[..]).sniff_mime(),
            None
        );
    }

    #[test]
    fn names_are_made_safe() {
        assert_eq!(sanitize_stem("My Logo (final).PNG"), "My-Logo-final");
        assert_eq!(sanitize_stem("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_stem("C:\\x\\y.png"), "y");
        assert_eq!(sanitize_stem("....png"), "file");
        assert_eq!(sanitize_stem("বাংলা.png"), "file");
        assert_eq!(sanitize_stem(&"a".repeat(200)).len(), 80);
    }

    #[test]
    fn extensions_follow_the_content_type() {
        assert_eq!(extension_for("image/jpeg"), "jpg");
        assert_eq!(extension_for("application/pdf"), "pdf");
    }
}
