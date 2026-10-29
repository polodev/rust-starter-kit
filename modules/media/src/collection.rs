/// Accepted content types for pictures.
pub const IMAGES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];
/// Pictures and PDF documents.
pub const DOCUMENTS: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/gif",
    "application/pdf",
];
/// 2 MiB, which is also Loco's default request body limit.
pub const DEFAULT_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionKind {
    /// Adding a file replaces the previous one (a logo, an avatar).
    Single,
    /// Many files; `limit` keeps only the newest N when set (a gallery).
    Multiple { limit: Option<usize> },
}

/// How one collection behaves, like Spatie's `MediaCollection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionDef {
    pub name: &'static str,
    pub kind: CollectionKind,
    /// Accepted content types; empty means any type this library can recognise.
    pub mime_types: &'static [&'static str],
    pub max_bytes: usize,
    /// Public files are served to everyone, private ones only to the owner and `media.view` staff.
    pub public: bool,
}

impl CollectionDef {
    /// A collection that holds exactly one file.
    pub const fn single(name: &'static str) -> Self {
        Self {
            name,
            kind: CollectionKind::Single,
            mime_types: &[],
            max_bytes: DEFAULT_MAX_BYTES,
            public: false,
        }
    }

    /// A collection that holds many files.
    pub const fn multiple(name: &'static str) -> Self {
        Self {
            name,
            kind: CollectionKind::Multiple { limit: None },
            mime_types: &[],
            max_bytes: DEFAULT_MAX_BYTES,
            public: false,
        }
    }

    /// Keep only the newest `count` files. `1` makes it a single-file collection, as in Spatie.
    pub const fn only_keep_latest(mut self, count: usize) -> Self {
        assert!(count >= 1, "only_keep_latest needs at least 1");
        self.kind = if count == 1 {
            CollectionKind::Single
        } else {
            CollectionKind::Multiple { limit: Some(count) }
        };
        self
    }

    pub const fn accepts_mime_types(mut self, mime_types: &'static [&'static str]) -> Self {
        self.mime_types = mime_types;
        self
    }

    pub const fn max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    pub const fn public(mut self) -> Self {
        self.public = true;
        self
    }

    pub const fn is_single_file(&self) -> bool {
        matches!(self.kind, CollectionKind::Single)
    }

    /// The most files this collection keeps, if limited.
    pub const fn keep_limit(&self) -> Option<usize> {
        match self.kind {
            CollectionKind::Single => Some(1),
            CollectionKind::Multiple { limit } => limit,
        }
    }

    pub fn accepts(&self, mime: &str) -> bool {
        self.mime_types.is_empty() || self.mime_types.contains(&mime)
    }

    /// Collections that were never declared behave like an unrestricted private multiple collection.
    pub const fn undeclared(name: &'static str) -> Self {
        Self::multiple(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_and_multiple_collections() {
        assert!(CollectionDef::single("logo").is_single_file());
        assert_eq!(CollectionDef::single("logo").keep_limit(), Some(1));
        let gallery = CollectionDef::multiple("gallery");
        assert!(!gallery.is_single_file());
        assert_eq!(gallery.keep_limit(), None);
        assert_eq!(gallery.only_keep_latest(10).keep_limit(), Some(10));
    }

    #[test]
    fn keeping_only_the_latest_one_means_single_file() {
        assert!(CollectionDef::multiple("x")
            .only_keep_latest(1)
            .is_single_file());
    }

    #[test]
    fn mime_rules() {
        let images = CollectionDef::single("logo").accepts_mime_types(IMAGES);
        assert!(images.accepts("image/png") && !images.accepts("application/pdf"));
        assert!(CollectionDef::multiple("any").accepts("application/pdf"));
    }
}
