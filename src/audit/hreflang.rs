//! Language alternates declared in the built HTML head.

use rustc_hash::FxHashSet;
use url::Url;

use super::{Audit, Code, absolute_web_url, issue};

struct LocalLink<'a> {
    source: &'a str,
    target: &'a str,
    url: Url,
    line: u32,
}

pub(super) fn check(audit: &mut Audit<'_>) {
    let mut links = Vec::new();
    let mut pairs = FxHashSet::default();

    for (&source, page) in &audit.pages {
        if page.metadata.alternates.is_empty() {
            continue;
        }
        let document_url = page.sitemap_url.as_ref().unwrap_or(&page.page_url);
        let mut has_self = false;
        for alternate in &page.metadata.alternates {
            let language = alternate.language.trim_ascii();
            let language_valid = valid_language(language);
            if !language_valid {
                audit.issues.push(issue(
                    Code::HreflangLanguageInvalid,
                    source,
                    alternate.line,
                    Some(document_url.as_str()),
                    format!("Invalid hreflang code {language:?}. Use a two-letter language, optional script and region, or x-default."),
                ));
            }
            let Some(url) = absolute_web_url(&alternate.href) else {
                audit.issues.push(issue(
                    Code::HreflangUrlInvalid,
                    source,
                    alternate.line,
                    Some(&alternate.href),
                    "Use an absolute HTTP(S) alternate URL without credentials or a fragment.",
                ));
                continue;
            };
            let Some(path) = audit.local_path(&url) else {
                // Only the declaration can be checked outside this deployment.
                continue;
            };
            let Some(target) = audit.site.serve(&path) else {
                audit.issues.push(issue(
                    Code::HreflangTargetMissing,
                    source,
                    alternate.line,
                    Some(url.as_str()),
                    "The hreflang URL does not resolve to a file in this build.",
                ));
                continue;
            };
            // Other file types can declare their alternates in HTTP headers.
            if !language_valid || !audit.pages.contains_key(target) {
                continue;
            }
            if source == target {
                has_self = true;
            } else {
                pairs.insert((source, target));
                links.push(LocalLink {
                    source,
                    target,
                    url,
                    line: alternate.line,
                });
            }
        }
        if !has_self {
            audit.issues.push(issue(
                Code::HreflangSelfMissing,
                source,
                0,
                Some(document_url.as_str()),
                "Add an hreflang declaration that points to this page.",
            ));
        }
    }

    for link in links {
        if !pairs.contains(&(link.target, link.source)) {
            audit.issues.push(issue(
                Code::HreflangReturnMissing,
                link.source,
                link.line,
                Some(link.url.as_str()),
                format!(
                    "The alternate page {} lacks an hreflang link back to this page.",
                    link.target
                ),
            ));
        }
    }
}

fn valid_language(value: &str) -> bool {
    if value.eq_ignore_ascii_case("x-default") {
        return true;
    }
    let mut subtags = value.split('-');
    let language = subtags.next().unwrap_or_default();
    if !registered(LANGUAGES, language) {
        return false;
    }
    match (subtags.next(), subtags.next(), subtags.next()) {
        (None, None, None) => true,
        (Some(subtag), None, None) => registered(SCRIPTS, subtag) || registered(REGIONS, subtag),
        (Some(script), Some(region), None) => {
            registered(SCRIPTS, script) && registered(REGIONS, region)
        }
        _ => false,
    }
}

fn registered(codes: &str, subtag: &str) -> bool {
    codes
        .split_ascii_whitespace()
        .any(|code| code.eq_ignore_ascii_case(subtag))
}

// Active two-letter language, script, and assigned alpha-2 region codes from
// the 2026-11-18 IANA registry snapshot. Region codes exclude private use and
// ISO 3166-1 reservations such as EU and CQ.
// https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry
const LANGUAGES: &str =
    "aa ab ae af ak am an ar as av ay az ba be bg bi bm bn bo br bs ca ce ch co cr cs cu cv
    cy da de dv dz ee el en eo es et eu fa ff fi fj fo fr fy ga gd gl gn gu gv ha he
    hi ho hr ht hu hy hz ia id ie ig ii ik io is it iu ja jv ka kg ki kj kk kl km kn
    ko kr ks ku kv kw ky la lb lg li ln lo lt lu lv mg mh mi mk ml mn mr ms mt my na
    nb nd ne ng nl nn no nr nv ny oc oj om or os pa pi pl ps pt qu rm rn ro ru rw sa
    sc sd se sg sh si sk sl sm sn so sq sr ss st su sv sw ta te tg th ti tk tl tn to
    tr ts tt tw ty ug uk ur uz ve vi vo wa wo xh yi yo za zh zu";

const SCRIPTS: &str =
    "Adlm Afak Aghb Ahom Arab Aran Armi Armn Avst Bali Bamu Bass Batk Beng Berf Bhks Blis
    Bopo Brah Brai Bugi Buhd Cakm Cans Cari Cham Cher Chis Chrs Cirt Copt Cpmn Cprt
    Cyrl Cyrs Deva Diak Dogr Dsrt Dupl Egyd Egyh Egyp Elba Elym Ethi Gara Geok Geor
    Glag Gong Gonm Goth Gran Grek Gujr Gukh Guru Hanb Hang Hani Hano Hans Hant Hatr
    Hebr Hira Hluw Hmng Hmnp Hntl Hrkt Hung Inds Ital Jamo Java Jpan Jurc Kali Kana
    Kawi Khar Khmr Khoj Kitl Kits Knda Kore Kpel Krai Kthi Lana Laoo Latf Latg Latn
    Leke Lepc Limb Lina Linb Lisu Loma Lyci Lydi Mahj Maka Mand Mani Marc Maya Medf
    Mend Merc Mero Mlym Modi Mong Moon Mroo Mtei Mult Mymr Nagm Nand Narb Nbat Newa
    Nkdb Nkgb Nkoo Nshu Ogam Olck Onao Orkh Orya Osge Osma Ougr Palm Pauc Pcun Pelm
    Perm Phag Phli Phlp Phlv Phnx Piqd Plrd Prti Psin Ranj Rjng Rohg Roro Runr Samr
    Sara Sarb Saur Seal Sgnw Shaw Shrd Shui Sidd Sidt Sind Sinh Sogd Sogo Sora Soyo
    Sund Sunu Sylo Syrc Syre Syrj Syrn Tagb Takr Tale Talu Taml Tang Tavt Tayo Telu
    Teng Tfng Tglg Thaa Thai Tibt Tirh Tnsa Todr Tols Toto Tutg Ugar Vaii Visp Vith
    Wara Wcho Wole Xpeo Xsux Yezi Yiii Zanb Zinh Zmth Zsye Zsym Zxxx Zyyy Zzzz";

const REGIONS: &str =
    "AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO
    BQ BR BS BT BV BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ
    DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA GB GD GE GF GG GH GI
    GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT
    JE JM JO JP KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA
    MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY MZ NA NC NE NF NG
    NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU
    RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH
    TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI VN VU WF WS YE
    YT ZA ZM ZW";

#[cfg(test)]
mod tests {
    use super::{absolute_web_url, valid_language};

    #[test]
    fn validates_registered_language_script_and_region_codes() {
        for value in ["de", "EN-gb", "zh-Hant", "sr-cyrl-RS", "X-Default"] {
            assert!(valid_language(value), "{value}");
        }
        for value in [
            "",
            "zz",
            "eng",
            "en_US",
            "en-UK",
            "en-EU",
            "en-CQ",
            "es-419",
            "zh-Abcd",
            "en-GB-Latn",
            "en-Latn-US-extra",
            "x-default-US",
        ] {
            assert!(!valid_language(value), "{value}");
        }
    }

    #[test]
    fn requires_fully_qualified_alternate_urls() {
        assert!(absolute_web_url("https://example.org/ardo/en/?view=full").is_some());
        for href in [
            "",
            "/ardo/en/",
            "//example.org/ardo/en/",
            "https:example.org/ardo/en/",
            "https:/example.org/ardo/en/?next=https://example.org/",
            "https:///example.org/ardo/en/",
            "mailto:de@example.org",
            "https://user@example.org/ardo/en/",
            "https://example.org/ardo/en/#overview",
        ] {
            assert!(absolute_web_url(href).is_none(), "{href}");
        }
    }
}
