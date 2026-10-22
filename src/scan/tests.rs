use super::{PageScan, decode_char_refs, refresh_url, scan, srcset_urls};

/// A document exercising every element ferrolink reads and every place a
/// link-shaped string must be ignored. Each start tag sits on one line so
/// that the reported lines are unambiguous.
const DOCUMENT: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<META HTTP-EQUIV="refresh" CONTENT="5; URL='/moved/'">
<meta property="og:image" content="https://example.org/og.png">
<meta name="description" content="/not-a-link">
<link rel="stylesheet" href="/assets/app.css">
<link rel="preconnect" href="https://fonts.example.org">
<script src="/assets/app.js"></script>
<script>document.write('<a href="/inside-script">');</script>
<style>a[href="/inside-style"] { color: red }</style>
<title>Docs <a href="/inside-title"></title>
</head>
<body id="top-of-page">
<!-- <a href="/inside-comment"> -->
<A HREF="/guide/?a=1&amp;b=2#intro" id="first">Guide</A>
<a href=relative/page.html>unquoted</a>
<a name="legacy-anchor"></a>
<a href="">empty</a>
<a href="mailto:team@example.org">mail</a>
<img src="/img/a.png" srcset="/img/a-1x.png 1x, /img/a,2x.png 2x">
<picture><source srcset="/img/b.webp"></picture>
<video src="/media/v.mp4" poster="/media/poster.jpg"><track src="/media/v.vtt"></video>
<iframe src="/embed/"></iframe>
<object data="/files/doc.pdf"></object>
<svg/><h2 id="after-svg">After</h2>
<svg viewBox="0 0 10 10"><title>Icon</title><use href="#symbol"></use><use xlink:href="/sprite.svg#icon"></use></svg>
<textarea><a href="/inside-textarea"></textarea>
<p id="caf&eacute;" title="it's fine">Text &amp; more</p>
<base href="/base/">
<base href="/second-base/">
</body>
</html>
"##;

fn expected() -> Vec<(&'static str, &'static str, u32)> {
    vec![
        ("/moved/", "meta[refresh]", 5),
        ("https://example.org/og.png", "meta[og:image]", 6),
        ("/assets/app.css", "link[href]", 8),
        ("/assets/app.js", "script[src]", 10),
        ("/guide/?a=1&b=2#intro", "a[href]", 17),
        ("relative/page.html", "a[href]", 18),
        ("mailto:team@example.org", "a[href]", 21),
        ("/img/a.png", "img[src]", 22),
        ("/img/a-1x.png", "img[srcset]", 22),
        ("/img/a,2x.png", "img[srcset]", 22),
        ("/img/b.webp", "source[srcset]", 23),
        ("/media/v.mp4", "video[src]", 24),
        ("/media/poster.jpg", "video[poster]", 24),
        ("/media/v.vtt", "track[src]", 24),
        ("/embed/", "iframe[src]", 25),
        ("/files/doc.pdf", "object[data]", 26),
        ("#symbol", "use[href]", 28),
        ("/sprite.svg#icon", "use[href]", 28),
    ]
}

fn links(scan: &PageScan) -> Vec<(&str, &'static str, u32)> {
    scan.links
        .iter()
        .map(|link| (&*link.url, link.origin, link.line))
        .collect()
}

fn urls(scan: &PageScan) -> Vec<&str> {
    scan.links.iter().map(|link| &*link.url).collect()
}

fn sorted_anchors(scan: &PageScan) -> Vec<&str> {
    let mut anchors: Vec<&str> = scan.anchors.iter().map(|anchor| &**anchor).collect();
    anchors.sort_unstable();
    anchors
}

#[test]
fn extracts_every_link_from_well_formed_markup() {
    let scan = scan(DOCUMENT.as_bytes());
    assert_eq!(links(&scan), expected());
    assert_eq!(scan.base_href.as_deref(), Some("/base/"));
}

#[test]
fn collects_ids_and_anchor_names() {
    let scan = scan(DOCUMENT.as_bytes());
    // Only the character references that occur in URLs are decoded; a
    // tokenizer would turn `&eacute;` into `é`.
    assert_eq!(
        sorted_anchors(&scan),
        [
            "after-svg",
            "caf&eacute;",
            "first",
            "legacy-anchor",
            "top-of-page"
        ]
    );
}

#[test]
fn resynchronizes_at_the_next_tag_after_a_stray_quote() {
    // To a browser the apostrophe in an unquoted value is just a character,
    // and both links count. The regex backend reads it as an unterminated
    // quoted value, gives up on the tag, and resumes at the next `<`.
    let html = br#"<a title=it's href="/real">x</a><a href="/next">y</a>"#;
    assert_eq!(urls(&scan(html)), ["/next"]);
}

#[test]
fn unterminated_comments_hide_the_rest_of_the_document() {
    let html = b"<a href=\"/before\"></a><!-- open <a href=\"/after\">";
    assert_eq!(urls(&scan(html)), ["/before"]);
}

#[test]
fn comment_and_raw_text_ends_follow_a_tokenizer() {
    let html = concat!(
        r#"<!--><a href="/after-empty-comment"></a>"#,
        r#"<!---><a href="/after-dash-comment"></a>"#,
        r#"<!-- a -- b --!><a href="/after-bang-comment"></a>"#,
        r#"<script>x</script foo="bar"><a href="/after-script"></a>"#,
        r#"<script-x src="/custom"></script-x><a href="/after-custom"></a>"#,
        r#"<style>a{}</styles></style><a href="/after-style"></a>"#,
    );
    assert_eq!(
        urls(&scan(html.as_bytes())),
        [
            "/after-empty-comment",
            "/after-dash-comment",
            "/after-bang-comment",
            "/after-script",
            "/after-custom",
            "/after-style"
        ]
    );
}

#[test]
fn reports_the_line_where_a_tag_starts() {
    let html = b"<p>\n\n<a\n href=\"/x\">\n</p>";
    assert_eq!(scan(html).links[0].line, 3);
}

#[test]
fn parses_srcset_candidates() {
    let urls: Vec<&str> = srcset_urls("a.png 1x,b.png 2x , c,d.png 100w,e.png,, f.png").collect();
    assert_eq!(urls, ["a.png", "b.png", "c,d.png", "e.png", "f.png"]);
    let urls: Vec<&str> = srcset_urls("img.png (max-width: 1px, 2px) 1x, other.png").collect();
    assert_eq!(urls, ["img.png", "other.png"]);
}

#[test]
fn parses_refresh_targets() {
    assert_eq!(refresh_url("0; url=/next"), Some("/next"));
    assert_eq!(refresh_url("3;URL = '/q'"), Some("/q"));
    assert_eq!(refresh_url("0, /bare"), Some("/bare"));
    assert_eq!(refresh_url("5"), None);
}

#[test]
fn decodes_character_references() {
    assert_eq!(decode_char_refs("/a?b=1&amp;c=2"), "/a?b=1&c=2");
    assert_eq!(decode_char_refs("&#47;x&#x2F;y"), "/x/y");
    assert_eq!(decode_char_refs("&unknown; & &amp"), "&unknown; & &amp");
}
