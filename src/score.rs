//! The guessing: turns a list of tabs into a close score per tab.
//!
//! No model is involved. Each rule that matches a tab adds or removes points
//! and leaves a short reason, so the window can say why a tab was suggested.
//! The score runs from 0 to 100; [`CLOSE_AT`] and above is suggested for
//! closing, [`MAYBE_AT`] and above is listed as worth a look.
//!
//! Everything here works on tab titles, since that is all Edge's tab strip
//! exposes. Nothing in this module touches the browser.

use std::collections::HashMap;

pub const CLOSE_AT: u8 = 60;
pub const MAYBE_AT: u8 = 35;

/// Whether Edge still holds the page in memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sleep {
    Loaded,
    Sleeping,
    Discarded,
}

/// One tab, with the notes Edge appends to its name split out.
#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub window: i64,
    pub index: usize,
    pub title: String,
    pub sleep: Sleep,
    pub memory_mb: Option<u32>,
    pub group: Option<String>,
    pub pinned: bool,
    pub split_view: bool,
    /// The tab on screen in its window.
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Close,
    Maybe,
    Keep,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub score: u8,
    pub reasons: Vec<String>,
}

impl Verdict {
    #[must_use]
    pub fn band(&self) -> Band {
        if self.score >= CLOSE_AT {
            Band::Close
        } else if self.score >= MAYBE_AT {
            Band::Maybe
        } else {
            Band::Keep
        }
    }
}

/// The user's own words from `rules.txt`: a title containing a `close` word
/// is always suggested, one containing a `keep` word never is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rules {
    pub close: Vec<String>,
    pub keep: Vec<String>,
}

impl Rules {
    /// Lines look like `close: gadget shop` or `keep: acme portal`. Blank lines and
    /// lines starting with `#` are ignored, as is anything else.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut rules = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((kind, word)) = line.split_once(':') else {
                continue;
            };
            let word = word.trim().to_lowercase();
            if word.is_empty() {
                continue;
            }
            match kind.trim().to_lowercase().as_str() {
                "close" => rules.close.push(word),
                "keep" => rules.keep.push(word),
                _ => {}
            }
        }
        rules
    }
}

/// Split the name Edge gives a tab into its title and the notes after it,
/// such as ` - Discarded - Memory usage - 296 MB`.
#[must_use]
pub fn parse_name(window: i64, index: usize, name: &str, active: bool) -> Tab {
    let mut rest = name.trim().to_string();
    let mut tab = Tab {
        window,
        index,
        title: String::new(),
        sleep: Sleep::Loaded,
        memory_mb: None,
        group: None,
        pinned: false,
        split_view: false,
        active,
    };
    loop {
        if tab.memory_mb.is_none() {
            if let Some((head, mb)) = strip_memory(&rest) {
                tab.memory_mb = Some(mb);
                rest = head;
                continue;
            }
        }
        if let Some(head) = rest.strip_suffix(" - Sleeping") {
            tab.sleep = Sleep::Sleeping;
            rest = head.to_string();
            continue;
        }
        if let Some(head) = rest.strip_suffix(" - Discarded") {
            tab.sleep = Sleep::Discarded;
            rest = head.to_string();
            continue;
        }
        if let Some(head) = rest
            .strip_suffix(" - Left view")
            .or_else(|| rest.strip_suffix(" - Right view"))
        {
            tab.split_view = true;
            rest = head.to_string();
            continue;
        }
        if let Some(head) = rest.strip_suffix(" - Pinned") {
            tab.pinned = true;
            rest = head.to_string();
            continue;
        }
        if tab.group.is_none() {
            const MARK: &str = " - Part of tab group ";
            if let Some(at) = rest.rfind(MARK) {
                tab.group = Some(rest[at + MARK.len()..].to_string());
                rest.truncate(at);
                continue;
            }
        }
        break;
    }
    tab.title = rest;
    tab
}

fn strip_memory(name: &str) -> Option<(String, u32)> {
    for mark in [" - High memory usage - ", " - Memory usage - "] {
        let Some(at) = name.rfind(mark) else {
            continue;
        };
        let Some(mb) = parse_size(&name[at + mark.len()..]) else {
            continue;
        };
        return Some((name[..at].to_string(), mb));
    }
    None
}

fn parse_size(text: &str) -> Option<u32> {
    let (number, unit) = text.trim().split_once(' ')?;
    let number: f32 = number.parse().ok()?;
    let mb = match unit {
        "KB" => number / 1024.0,
        "MB" => number,
        "GB" => number * 1024.0,
        _ => return None,
    };
    if !mb.is_finite() || mb < 0.0 {
        return None;
    }
    // Truncation is fine: this is a size shown to the nearest megabyte.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(mb.round() as u32)
}

/// Score every tab. The result is in the same order as `tabs`.
#[must_use]
pub fn score_all(tabs: &[Tab], rules: &Rules) -> Vec<Verdict> {
    let duplicates = duplicate_flags(tabs);
    let crowds = site_counts(tabs);
    tabs.iter()
        .zip(duplicates)
        .map(|(tab, duplicate)| {
            let crowd = site_key(&tab.title)
                .and_then(|site| crowds.get(&site.to_lowercase()).map(|n| (site, *n)));
            score_one(tab, duplicate, crowd, rules)
        })
        .collect()
}

fn score_one(tab: &Tab, duplicate: bool, crowd: Option<(&str, usize)>, rules: &Rules) -> Verdict {
    let title = tab.title.to_lowercase();
    let mut points: i32 = 0;
    let mut reasons = Vec::new();
    let mut add = |amount: i32, reason: String| {
        points += amount;
        reasons.push(reason);
    };

    if let Some(word) = rules.keep.iter().find(|word| title.contains(*word)) {
        return Verdict {
            score: 0,
            reasons: vec![format!("rules.txt keeps \"{word}\"")],
        };
    }
    if let Some(word) = rules.close.iter().find(|word| title.contains(*word)) {
        add(70, format!("rules.txt closes \"{word}\""));
    }

    if title.is_empty() || BLANK.contains(&title.as_str()) {
        add(70, "blank tab".to_string());
    } else if contains_any(&title, ERROR) {
        add(80, "error page".to_string());
    } else if title.starts_with("view-source:") {
        add(60, "page source view".to_string());
    } else if is_bare_image(&title) {
        add(55, "a single image".to_string());
    } else if contains_any(&title, ONE_OFF) {
        add(50, "one-off notice".to_string());
    } else if contains_any(&title, SIGN_IN) {
        add(45, "sign-in page".to_string());
    } else if ends_with_any(&title, SEARCH) {
        add(40, "old search".to_string());
    }

    if duplicate {
        add(60, "duplicate of another tab".to_string());
    }
    if ends_with_any(&title, FEEDS) {
        add(25, "social feed".to_string());
    }
    if title.starts_with("buy ") || contains_any(&title, SHOPS) {
        add(20, "shopping".to_string());
    }
    match tab.sleep {
        Sleep::Discarded => add(15, "discarded, not used lately".to_string()),
        Sleep::Sleeping => add(8, "sleeping".to_string()),
        Sleep::Loaded => {}
    }
    if let Some(mb) = tab.memory_mb.filter(|mb| *mb >= 500) {
        add(10, format!("heavy, {}", memory_text(mb)));
    }
    if let Some((site, count)) = crowd.filter(|(_, count)| *count >= 4) {
        add(10, format!("{count} tabs from {site}"));
    }

    if tab.pinned {
        add(-60, "pinned".to_string());
    }
    if tab.active {
        add(-40, "on screen now".to_string());
    }
    if tab.group.is_some() {
        add(-25, "in a tab group".to_string());
    }
    if tab.split_view {
        add(-10, "in a split view".to_string());
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let score = points.clamp(0, 100) as u8;
    Verdict { score, reasons }
}

const BLANK: &[&str] = &["new tab", "untitled", "about:blank", "blank page"];

const ERROR: &[&str] = &[
    "network error",
    "can't be reached",
    "can\u{2019}t be reached",
    "invalid ssl",
    "bad gateway",
    "gateway time-out",
    "gateway timeout",
    "service unavailable",
    "page not found",
    "404 not found",
    "isn't working",
    "isn\u{2019}t working",
    "privacy error",
];

const ONE_OFF: &[&str] = &[
    "installation successful",
    "successfully installed",
    "thanks for installing",
    "thank you for installing",
    "has been paused",
    "upgraded - ",
    "unsubscribe",
];

const SIGN_IN: &[&str] = &[
    "sign in",
    "sign-in",
    "log in",
    "login",
    "two factor",
    "two-factor",
    "verify your",
];

const SEARCH: &[&str] = &[
    " - google search",
    " - search",
    " at duckduckgo",
    " - bing",
    " - brave search",
];

const FEEDS: &[&str] = &[
    "/ x",
    "| linkedin",
    "| facebook",
    " - youtube",
    " - reddit",
    "| instagram",
];

const SHOPS: &[&str] = &["amazon.", "ebay", "etsy", "aliexpress"];

fn contains_any(title: &str, words: &[&str]) -> bool {
    words.iter().any(|word| title.contains(word))
}

fn ends_with_any(title: &str, endings: &[&str]) -> bool {
    endings.iter().any(|ending| title.ends_with(ending))
}

/// A tab showing one image file is titled like `logo.png (616×237)`.
fn is_bare_image(title: &str) -> bool {
    let Some((file, size)) = title.rsplit_once(" (") else {
        return false;
    };
    let is_size = size.ends_with(')') && size.contains('\u{d7}');
    let is_image = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"]
        .iter()
        .any(|ext| file.ends_with(ext));
    is_size && is_image
}

/// For each tab, whether an earlier or better copy of the same title exists.
/// Of a set of copies, the one on screen is kept, then one that is pinned or
/// in a tab group, then a loaded one, then the first.
fn duplicate_flags(tabs: &[Tab]) -> Vec<bool> {
    let mut sets: HashMap<String, Vec<usize>> = HashMap::new();
    for (at, tab) in tabs.iter().enumerate() {
        if !tab.title.is_empty() {
            sets.entry(tab.title.to_lowercase()).or_default().push(at);
        }
    }
    let mut flags = vec![false; tabs.len()];
    for copies in sets.values().filter(|copies| copies.len() > 1) {
        let keeper = copies
            .iter()
            .find(|at| tabs[**at].active)
            .or_else(|| {
                copies
                    .iter()
                    .find(|at| tabs[**at].pinned || tabs[**at].group.is_some())
            })
            .or_else(|| copies.iter().find(|at| tabs[**at].sleep == Sleep::Loaded))
            .unwrap_or(&copies[0]);
        for at in copies {
            flags[*at] = at != keeper;
        }
    }
    flags
}

/// The site name most pages put last in their title: `Pull requests · acme`
/// gives `acme`. `None` when the title has no separator or the last part is
/// too long to be a name.
#[must_use]
pub fn site_key(title: &str) -> Option<&str> {
    let mut last = None;
    for separator in [" | ", " - ", " \u{2013} ", " \u{2014} ", " \u{b7} "] {
        if let Some(at) = title.rfind(separator) {
            let start = at + separator.len();
            if last.is_none_or(|best| start > best) {
                last = Some(start);
            }
        }
    }
    let site = title[last?..].trim();
    (!site.is_empty() && site.chars().count() <= 32).then_some(site)
}

fn site_counts(tabs: &[Tab]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for tab in tabs {
        if let Some(site) = site_key(&tab.title) {
            *counts.entry(site.to_lowercase()).or_insert(0) += 1;
        }
    }
    counts
}

/// `296 MB`, or `1.1 GB` from a gigabyte up.
#[must_use]
pub fn memory_text(mb: u32) -> String {
    if mb >= 1024 {
        format!("{:.1} GB", f64::from(mb) / 1024.0)
    } else {
        format!("{mb} MB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(index: usize, name: &str) -> Tab {
        parse_name(1, index, name, false)
    }

    fn score(names: &[&str]) -> Vec<Verdict> {
        let tabs: Vec<Tab> = names
            .iter()
            .enumerate()
            .map(|(index, name)| tab(index, name))
            .collect();
        score_all(&tabs, &Rules::default())
    }

    #[test]
    fn the_notes_edge_appends_are_split_from_the_title() {
        let parsed = tab(
            0,
            "Dashboard \u{2013} Payments [Test] - Part of tab group Payments setup - Discarded - Memory usage - 725 MB",
        );
        assert_eq!(parsed.title, "Dashboard \u{2013} Payments [Test]");
        assert_eq!(parsed.group.as_deref(), Some("Payments setup"));
        assert_eq!(parsed.sleep, Sleep::Discarded);
        assert_eq!(parsed.memory_mb, Some(725));
    }

    #[test]
    fn gigabytes_and_split_views_are_read() {
        let parsed = tab(
            0,
            "Laptop buying - Notes - Left view - Sleeping - High memory usage - 1.1 GB",
        );
        assert_eq!(parsed.title, "Laptop buying - Notes");
        assert!(parsed.split_view);
        assert_eq!(parsed.sleep, Sleep::Sleeping);
        assert_eq!(parsed.memory_mb, Some(1126));
    }

    #[test]
    fn a_plain_title_is_left_alone() {
        let parsed = tab(0, "Incidents - Status Board");
        assert_eq!(parsed.title, "Incidents - Status Board");
        assert_eq!(parsed.sleep, Sleep::Loaded);
        assert_eq!(parsed.memory_mb, None);
    }

    #[test]
    fn error_pages_are_suggested_for_closing() {
        let verdicts = score(&[
            "example.com - Network error",
            "example.org | 526: Invalid SSL certificate",
        ]);
        assert!(verdicts.iter().all(|verdict| verdict.band() == Band::Close));
    }

    #[test]
    fn of_two_copies_the_loaded_one_is_kept() {
        let verdicts = score(&["Dashboard - Discarded - Memory usage - 150 MB", "Dashboard"]);
        assert_eq!(verdicts[0].band(), Band::Close);
        assert_eq!(verdicts[1].band(), Band::Keep);
    }

    #[test]
    fn the_copy_on_screen_is_kept_over_a_loaded_one() {
        let tabs = vec![
            parse_name(1, 0, "Timesheet", false),
            parse_name(1, 1, "Timesheet", true),
        ];
        let verdicts = score_all(&tabs, &Rules::default());
        assert_eq!(verdicts[0].band(), Band::Close);
        assert_eq!(verdicts[1].score, 0);
    }

    #[test]
    fn a_working_tab_is_kept() {
        let verdicts = score(&[
            "Incidents - Status Board",
            "Pull requests \u{b7} acme/portal - Discarded - Memory usage - 142 MB",
        ]);
        assert!(verdicts.iter().all(|verdict| verdict.band() == Band::Keep));
    }

    #[test]
    fn a_stale_search_is_worth_a_look_and_scores_higher_once_discarded() {
        let verdicts = score(&[
            "laptop reviews - Google Search",
            "monitor reviews - Google Search - Discarded - Memory usage - 143 MB",
        ]);
        assert_eq!(verdicts[0].band(), Band::Maybe);
        assert_eq!(verdicts[1].band(), Band::Maybe);
        assert!(verdicts[1].score > verdicts[0].score);
    }

    #[test]
    fn of_two_copies_the_one_in_a_tab_group_is_kept() {
        let verdicts = score(&[
            "Console | Developer Portal",
            "Console | Developer Portal - Part of tab group Email checks - Discarded - Memory usage - 80.5 MB",
        ]);
        assert_eq!(verdicts[0].band(), Band::Close);
        assert_eq!(verdicts[1].band(), Band::Keep);
    }

    #[test]
    fn a_bare_image_is_recognised() {
        assert!(is_bare_image("logo_616x237.png (616\u{d7}237)"));
        assert!(!is_bare_image("Asset 1"));
        assert!(!is_bare_image("Notes (draft)"));
    }

    #[test]
    fn the_site_is_the_last_part_of_the_title() {
        assert_eq!(site_key("Alex Example | LinkedIn"), Some("LinkedIn"));
        assert_eq!(site_key("Copier Paper - Supplies"), Some("Supplies"));
        assert_eq!(site_key("Home \u{2014} Acme Parts"), Some("Acme Parts"));
        assert_eq!(site_key("Timesheet"), None);
    }

    #[test]
    fn four_tabs_from_one_site_are_noted() {
        let verdicts = score(&[
            "A | LinkedIn",
            "B | LinkedIn",
            "C | LinkedIn",
            "D | LinkedIn",
        ]);
        assert!(verdicts[0]
            .reasons
            .iter()
            .any(|reason| reason == "4 tabs from LinkedIn"));
    }

    #[test]
    fn rules_file_words_win() {
        let rules = Rules::parse("# mine\nclose: gadget shop\nkeep: Acme Portal\nnonsense\n");
        assert_eq!(rules.close, ["gadget shop"]);
        assert_eq!(rules.keep, ["acme portal"]);

        let tabs = vec![
            tab(0, "Phones - Gadget Shop"),
            tab(
                1,
                "Two factor authentication - Acme Portal - Discarded - Memory usage - 61.7 MB",
            ),
        ];
        let verdicts = score_all(&tabs, &rules);
        assert_eq!(verdicts[0].band(), Band::Close);
        assert_eq!(verdicts[1].score, 0);
    }

    #[test]
    fn memory_is_shown_in_the_larger_unit() {
        assert_eq!(memory_text(296), "296 MB");
        assert_eq!(memory_text(1126), "1.1 GB");
    }
}
