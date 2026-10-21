//! The files a build produced and how a static host maps URL paths onto them.

use rustc_hash::FxHashSet;

/// Every file of the build, keyed by its `/`-separated path relative to the
/// site root.
pub(crate) struct Site {
    files: FxHashSet<Box<str>>,
}

impl Site {
    pub(crate) fn new(files: impl IntoIterator<Item = Box<str>>) -> Self {
        Self {
            files: files.into_iter().collect(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.files.len()
    }

    pub(crate) fn contains(&self, key: &str) -> bool {
        self.files.contains(key)
    }

    /// The file a static host serves for a decoded, site-relative URL path,
    /// such as `guide/setup` or `assets/app.css`.
    ///
    /// Follows what GitHub Pages, Netlify, Cloudflare Pages, and most static
    /// servers have in common: a trailing slash serves the directory's
    /// `index.html`; otherwise the exact file wins, then `<path>.html`, then
    /// `<path>/index.html` (reached through a redirect to the slash form).
    /// Lookups are case-sensitive, as on those hosts, whatever the local file
    /// system does.
    pub(crate) fn serve(&self, path: &str) -> Option<&str> {
        let found = |candidate: &str| self.files.get(candidate).map(|key| &**key);
        if path.is_empty() || path.ends_with('/') {
            return found(&format!("{path}index.html"))
                .or_else(|| found(&format!("{path}index.htm")));
        }
        found(path)
            .or_else(|| found(&format!("{path}.html")))
            .or_else(|| found(&format!("{path}/index.html")))
    }
}

#[cfg(test)]
mod tests {
    use super::Site;

    fn site(files: &[&str]) -> Site {
        Site::new(files.iter().map(|file| Box::from(*file)))
    }

    #[test]
    fn serves_directory_indexes_and_extensionless_pages() {
        let site = site(&["index.html", "guide/index.html", "about.html", "app.css"]);
        assert_eq!(site.serve(""), Some("index.html"));
        assert_eq!(site.serve("guide/"), Some("guide/index.html"));
        assert_eq!(site.serve("guide"), Some("guide/index.html"));
        assert_eq!(site.serve("about"), Some("about.html"));
        assert_eq!(site.serve("about.html"), Some("about.html"));
        assert_eq!(site.serve("app.css"), Some("app.css"));
    }

    #[test]
    fn rejects_missing_files_and_case_mismatches() {
        let site = site(&["guide/index.html", "Logo.svg"]);
        assert_eq!(site.serve("guide/setup"), None);
        assert_eq!(site.serve("about/"), None);
        assert_eq!(site.serve("logo.svg"), None);
        // A file is not a directory.
        assert_eq!(site.serve("Logo.svg/"), None);
    }
}
