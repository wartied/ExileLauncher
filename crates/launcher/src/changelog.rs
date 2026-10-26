//! Release notes for the changelog in Settings. They come from the GitHub
//! releases (written from the tag message when a version is released) and a
//! copy is kept so the list still shows offline.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use crate::{net, paths};

const RELEASES: &str = "https://api.github.com/repos/wartied/ExileLauncher/releases?per_page=30";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    /// Without the leading "v", e.g. "0.2.2".
    pub version: String,
    /// Display date, e.g. "25 Oct 2026".
    pub date: String,
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Note {
    Bullet(String),
    Text(String),
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

fn cache_file() -> PathBuf {
    paths::root().join("changelog.json")
}

/// Newest first. Falls back to the saved copy when GitHub can't be reached.
pub fn load() -> Result<Vec<Release>> {
    match fetch() {
        Ok(releases) => {
            if let Ok(json) = serde_json::to_vec(&releases) {
                let _ = std::fs::write(cache_file(), json);
            }
            Ok(releases)
        }
        Err(e) => cached().ok_or(e),
    }
}

pub fn cached() -> Option<Vec<Release>> {
    serde_json::from_slice(&std::fs::read(cache_file()).ok()?).ok()
}

fn fetch() -> Result<Vec<Release>> {
    let releases: Vec<GithubRelease> = net::get_json(RELEASES).context("loading release notes")?;
    Ok(releases.into_iter().filter(|r| !r.draft && !r.prerelease).map(convert).collect())
}

fn convert(r: GithubRelease) -> Release {
    Release {
        version: r.tag_name.trim_start_matches('v').to_owned(),
        date: r.published_at.as_deref().map(display_date).unwrap_or_default(),
        notes: parse_notes(r.body.as_deref().unwrap_or("")),
    }
}

/// "2026-10-25T09:20:45Z" -> "25 Oct 2026".
fn display_date(iso: &str) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut parts = iso.get(..10).unwrap_or("").split('-');
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
        return String::new();
    };
    match (m.parse::<usize>(), d.parse::<u32>()) {
        (Ok(m @ 1..=12), Ok(d)) => format!("{d} {} {y}", MONTHS[m - 1]),
        _ => String::new(),
    }
}

/// Markdown-ish notes to lines: "- " and "* " start bullets, headings and
/// other lines are plain text, emphasis markers are dropped.
fn parse_notes(body: &str) -> Vec<Note> {
    let mut notes = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        let (bullet, rest) = match line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            Some(rest) => (true, rest),
            None => (false, line.trim_start_matches('#')),
        };
        let rest = rest.replace("**", "").replace('`', "");
        let rest = rest.trim();
        if rest.is_empty() || rest == "-" {
            continue;
        }
        let mut rest = rest.to_owned();
        if let Some(first) = rest.get(..1) {
            rest.replace_range(..1, &first.to_uppercase());
        }
        notes.push(if bullet { Note::Bullet(rest) } else { Note::Text(rest) });
    }
    if notes.is_empty() {
        notes.push(Note::Text("Bug fixes and improvements.".into()));
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_become_lines() {
        let notes = parse_notes("## New\n- settings menu\n* **account** switcher\n-\n\nthanks for playing\n");
        assert_eq!(
            notes,
            [
                Note::Text("New".into()),
                Note::Bullet("Settings menu".into()),
                Note::Bullet("Account switcher".into()),
                Note::Text("Thanks for playing".into()),
            ]
        );
        assert_eq!(parse_notes("\n\n"), [Note::Text("Bug fixes and improvements.".into())]);
    }

    #[test]
    fn dates_read_well() {
        assert_eq!(display_date("2026-10-25T09:20:45Z"), "25 Oct 2026");
        assert_eq!(display_date("2026-01-05T00:00:00Z"), "5 Jan 2026");
        assert_eq!(display_date("nonsense"), "");
    }

    #[test]
    fn releases_from_github() {
        let json = r#"[{"tag_name":"v0.2.2","published_at":"2026-10-25T09:20:45Z","body":"- a\n- b"},
                       {"tag_name":"v0.3.0-rc","prerelease":true},{"tag_name":"v0.2.1","body":null}]"#;
        let raw: Vec<GithubRelease> = serde_json::from_str(json).unwrap();
        let releases: Vec<Release> = raw.into_iter().filter(|r| !r.draft && !r.prerelease).map(convert).collect();
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].version, "0.2.2");
        assert_eq!(releases[0].notes.len(), 2);
        assert_eq!(releases[1].date, "");
    }
}
