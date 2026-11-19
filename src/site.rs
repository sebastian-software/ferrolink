//! The files a build produced and how a static host maps URL paths onto them.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ferralk::{WalkOptions, Walker};
use rustc_hash::FxHashSet;

use crate::check::Error;

pub(crate) struct PageFile {
    pub key: Box<str>,
    pub path: PathBuf,
}

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

    pub(crate) fn validate_root(root: &Path) -> Result<(), Error> {
        let metadata = fs::metadata(root).map_err(|error| Error::Root(root.to_owned(), error))?;
        if !metadata.is_dir() {
            let error = io::Error::new(io::ErrorKind::NotADirectory, "not a directory");
            return Err(Error::Root(root.to_owned(), error));
        }
        Ok(())
    }

    pub(crate) fn walk(root: &Path, threads: usize) -> Result<(Self, Vec<PageFile>), Error> {
        let walk = Walker::new(root)
            .match_hidden(true)
            .threads(threads)
            .options(WalkOptions::default().files_only(true))
            .collect()
            .map_err(|error| Error::Walk(describe(&error)))?;
        let (entries, errors) = walk.into_parts();
        if let Some(error) = errors.first() {
            return Err(Error::Walk(describe(error)));
        }
        let mut keys = Vec::with_capacity(entries.len());
        let mut pages = Vec::new();
        for entry in &entries {
            let relative = entry.relative_path();
            let key = site_key(relative);
            if is_html(&key) {
                pages.push(PageFile {
                    key: key.clone(),
                    path: relative.to_path_buf(),
                });
            }
            keys.push(key);
        }
        // Walk order depends on thread scheduling.
        pages.sort_unstable_by(|a, b| a.key.cmp(&b.key));
        Ok((Self::new(keys), pages))
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
    /// `index.html` (or `index.htm`); otherwise the exact file wins, then
    /// `<path>.html`, then the directory index reached through the slash form.
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
            .or_else(|| found(&format!("{path}/index.htm")))
    }
}

fn site_key(relative: &Path) -> Box<str> {
    let mut key = String::new();
    for component in relative.components() {
        if !key.is_empty() {
            key.push('/');
        }
        key.push_str(&component.as_os_str().to_string_lossy());
    }
    key.into()
}

fn is_html(key: &str) -> bool {
    let extension = key.rsplit_once('.').map_or("", |(_, extension)| extension);
    extension.eq_ignore_ascii_case("html") || extension.eq_ignore_ascii_case("htm")
}

fn describe(error: &dyn std::error::Error) -> String {
    match error.source() {
        Some(cause) => format!("{error}: {cause}"),
        None => error.to_string(),
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
    fn serves_htm_directory_indexes_after_existing_candidates() {
        let site = site(&[
            "guide/index.htm",
            "both/index.htm",
            "both/index.html",
            "about/index.htm",
            "about.html",
            "exact/index.htm",
            "exact.html",
            "exact",
        ]);
        assert_eq!(site.serve("guide"), Some("guide/index.htm"));
        assert_eq!(site.serve("guide/"), Some("guide/index.htm"));
        assert_eq!(site.serve("both"), Some("both/index.html"));
        assert_eq!(site.serve("both/"), Some("both/index.html"));
        assert_eq!(site.serve("about"), Some("about.html"));
        assert_eq!(site.serve("exact"), Some("exact"));
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
