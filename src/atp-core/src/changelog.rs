//! # Git Changelog Generator
//!
//! Parses Conventional Commits, suggests semantic version bumps, detects
//! breaking changes, groups by scope, and outputs Markdown or JSON.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Conventional Commit types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CommitType {
    Feature,
    Fix,
    Docs,
    Style,
    Refactor,
    Perf,
    Test,
    Build,
    Ci,
    Chore,
    Revert,
    /// Unknown / non-conventional commit type.
    Other,
}

impl CommitType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommitType::Feature => "feat",
            CommitType::Fix => "fix",
            CommitType::Docs => "docs",
            CommitType::Style => "style",
            CommitType::Refactor => "refactor",
            CommitType::Perf => "perf",
            CommitType::Test => "test",
            CommitType::Build => "build",
            CommitType::Ci => "ci",
            CommitType::Chore => "chore",
            CommitType::Revert => "revert",
            CommitType::Other => "other",
        }
    }

    pub fn heading(&self) -> &'static str {
        match self {
            CommitType::Feature => "Features",
            CommitType::Fix => "Bug Fixes",
            CommitType::Docs => "Documentation",
            CommitType::Style => "Styles",
            CommitType::Refactor => "Code Refactoring",
            CommitType::Perf => "Performance Improvements",
            CommitType::Test => "Tests",
            CommitType::Build => "Build System",
            CommitType::Ci => "Continuous Integration",
            CommitType::Chore => "Chores",
            CommitType::Revert => "Reverts",
            CommitType::Other => "Other Changes",
        }
    }

    fn from_str_type(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "feat" | "feature" => CommitType::Feature,
            "fix" | "bugfix" => CommitType::Fix,
            "docs" | "doc" => CommitType::Docs,
            "style" => CommitType::Style,
            "refactor" => CommitType::Refactor,
            "perf" | "performance" => CommitType::Perf,
            "test" | "tests" => CommitType::Test,
            "build" => CommitType::Build,
            "ci" => CommitType::Ci,
            "chore" => CommitType::Chore,
            "revert" => CommitType::Revert,
            _ => CommitType::Other,
        }
    }
}

/// A parsed conventional commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConventionalCommit {
    /// Commit type (feat, fix, etc.).
    pub commit_type: CommitType,
    /// Optional scope in parentheses.
    pub scope: Option<String>,
    /// Whether this is a breaking change (! suffix or BREAKING CHANGE footer).
    pub breaking: bool,
    /// The commit description (first line after type).
    pub description: String,
    /// The full commit body (everything after first line).
    pub body: Option<String>,
    /// Footer key-value pairs (e.g., `BREAKING CHANGE: ...`).
    pub footers: BTreeMap<String, String>,
    /// Original raw commit message.
    pub raw: String,
    /// Commit hash (if available).
    pub hash: Option<String>,
    /// Author name (if available).
    pub author: Option<String>,
    /// Commit date (if available).
    pub date: Option<String>,
}

/// Semantic version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre: Option<String>,
}

impl SemVer {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            pre: None,
        }
    }

    pub fn parse(s: &str) -> Option<SemVer> {
        let s = s.strip_prefix('v').unwrap_or(s);
        let (version_part, pre) = if let Some((v, p)) = s.split_once('-') {
            (v, Some(p.to_string()))
        } else {
            (s, None)
        };
        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(SemVer {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
            pre,
        })
    }

    pub fn bump_major(&self) -> SemVer {
        SemVer::new(self.major + 1, 0, 0)
    }

    pub fn bump_minor(&self) -> SemVer {
        SemVer::new(self.major, self.minor + 1, 0)
    }

    pub fn bump_patch(&self) -> SemVer {
        SemVer::new(self.major, self.minor, self.patch + 1)
    }

    pub fn to_string_v(&self) -> String {
        let base = format!("{}.{}.{}", self.major, self.minor, self.patch);
        if let Some(ref pre) = self.pre {
            format!("{base}-{pre}")
        } else {
            base
        }
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_v())
    }
}

/// Suggested version bump kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BumpKind {
    Major,
    Minor,
    Patch,
    None,
}

/// Changelog entry for a version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    /// Version string.
    pub version: String,
    /// Date string (e.g., "2026-03-06").
    pub date: Option<String>,
    /// Commits grouped by type.
    pub sections: BTreeMap<CommitType, Vec<ConventionalCommit>>,
    /// Breaking changes extracted.
    pub breaking_changes: Vec<String>,
}

/// Changelog generation statistics.
#[derive(Debug, Clone, Default)]
pub struct ChangelogStats {
    pub total_commits: usize,
    pub conventional_commits: usize,
    pub breaking_changes: usize,
    pub commits_by_type: BTreeMap<CommitType, usize>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a single commit message as a Conventional Commit.
pub fn parse_commit(message: &str) -> ConventionalCommit {
    let re = Regex::new(
        r"^(?P<type>[a-zA-Z]+)(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?:\s*(?P<desc>.+)",
    )
    .unwrap();

    let first_line = message.lines().next().unwrap_or("");
    let body_text = message.lines().skip(1).collect::<Vec<_>>().join("\n");
    let body_trimmed = body_text.trim();

    if let Some(caps) = re.captures(first_line) {
        let commit_type = CommitType::from_str_type(&caps["type"]);
        let scope = caps.name("scope").map(|m| m.as_str().to_string());
        let breaking_mark = caps.name("breaking").is_some();
        let description = caps["desc"].trim().to_string();

        // Parse footers
        let mut footers = BTreeMap::new();
        let mut body_lines = Vec::new();
        let mut in_footer = false;

        for line in body_trimmed.lines() {
            if let Some((key, val)) = line.split_once(": ") {
                if key
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == ' ')
                {
                    footers.insert(key.to_string(), val.to_string());
                    in_footer = true;
                    continue;
                }
            }
            if !in_footer {
                body_lines.push(line);
            }
        }

        let breaking = breaking_mark
            || footers.contains_key("BREAKING CHANGE")
            || footers.contains_key("BREAKING-CHANGE");

        ConventionalCommit {
            commit_type,
            scope,
            breaking,
            description,
            body: if body_lines.is_empty() {
                None
            } else {
                Some(body_lines.join("\n"))
            },
            footers,
            raw: message.to_string(),
            hash: None,
            author: None,
            date: None,
        }
    } else {
        ConventionalCommit {
            commit_type: CommitType::Other,
            scope: None,
            breaking: false,
            description: first_line.to_string(),
            body: if body_trimmed.is_empty() {
                None
            } else {
                Some(body_trimmed.to_string())
            },
            footers: BTreeMap::new(),
            raw: message.to_string(),
            hash: None,
            author: None,
            date: None,
        }
    }
}

/// Parse multiple commit messages.
pub fn parse_commits(messages: &[&str]) -> Vec<ConventionalCommit> {
    messages.iter().map(|m| parse_commit(m)).collect()
}

// ---------------------------------------------------------------------------
// Version bump suggestion
// ---------------------------------------------------------------------------

/// Suggest a version bump based on a set of commits.
pub fn suggest_bump(commits: &[ConventionalCommit]) -> BumpKind {
    let has_breaking = commits.iter().any(|c| c.breaking);
    let has_feature = commits.iter().any(|c| c.commit_type == CommitType::Feature);
    let has_fix = commits.iter().any(|c| {
        matches!(
            c.commit_type,
            CommitType::Fix | CommitType::Perf | CommitType::Refactor
        )
    });

    if has_breaking {
        BumpKind::Major
    } else if has_feature {
        BumpKind::Minor
    } else if has_fix {
        BumpKind::Patch
    } else {
        BumpKind::None
    }
}

/// Apply a bump to a version.
pub fn apply_bump(version: &SemVer, bump: BumpKind) -> SemVer {
    match bump {
        BumpKind::Major => version.bump_major(),
        BumpKind::Minor => version.bump_minor(),
        BumpKind::Patch => version.bump_patch(),
        BumpKind::None => version.clone(),
    }
}

// ---------------------------------------------------------------------------
// Changelog generation
// ---------------------------------------------------------------------------

/// Generate a changelog entry from commits.
pub fn generate_changelog(
    version: &str,
    date: Option<&str>,
    commits: &[ConventionalCommit],
) -> ChangelogEntry {
    let mut sections: BTreeMap<CommitType, Vec<ConventionalCommit>> = BTreeMap::new();
    let mut breaking_changes = Vec::new();

    for commit in commits {
        sections
            .entry(commit.commit_type)
            .or_default()
            .push(commit.clone());
        if commit.breaking {
            let bc_detail = commit
                .footers
                .get("BREAKING CHANGE")
                .or_else(|| commit.footers.get("BREAKING-CHANGE"))
                .cloned()
                .unwrap_or_else(|| commit.description.clone());
            breaking_changes.push(bc_detail);
        }
    }

    ChangelogEntry {
        version: version.to_string(),
        date: date.map(|d| d.to_string()),
        sections,
        breaking_changes,
    }
}

/// Render a changelog entry as Markdown.
pub fn render_markdown(entry: &ChangelogEntry) -> String {
    let mut out = String::new();
    let date_str = entry.date.as_deref().unwrap_or("UNRELEASED");
    out.push_str(&format!("## [{}] — {}\n\n", entry.version, date_str));

    if !entry.breaking_changes.is_empty() {
        out.push_str("### ⚠ BREAKING CHANGES\n\n");
        for bc in &entry.breaking_changes {
            out.push_str(&format!("- {bc}\n"));
        }
        out.push('\n');
    }

    // Render sections in a consistent order
    let type_order = [
        CommitType::Feature,
        CommitType::Fix,
        CommitType::Perf,
        CommitType::Refactor,
        CommitType::Docs,
        CommitType::Style,
        CommitType::Test,
        CommitType::Build,
        CommitType::Ci,
        CommitType::Chore,
        CommitType::Revert,
        CommitType::Other,
    ];

    for ct in &type_order {
        if let Some(commits) = entry.sections.get(ct) {
            out.push_str(&format!("### {}\n\n", ct.heading()));
            for c in commits {
                let scope_str = c
                    .scope
                    .as_ref()
                    .map(|s| format!("**{s}**: "))
                    .unwrap_or_default();
                out.push_str(&format!("- {scope_str}{}\n", c.description));
            }
            out.push('\n');
        }
    }

    out
}

/// Render a changelog entry as JSON.
pub fn render_json(entry: &ChangelogEntry) -> String {
    serde_json::to_string_pretty(entry).unwrap_or_else(|_| "{}".to_string())
}

/// Compute changelog statistics.
pub fn changelog_stats(commits: &[ConventionalCommit]) -> ChangelogStats {
    let mut stats = ChangelogStats {
        total_commits: commits.len(),
        ..Default::default()
    };
    for c in commits {
        if c.commit_type != CommitType::Other {
            stats.conventional_commits += 1;
        }
        if c.breaking {
            stats.breaking_changes += 1;
        }
        *stats.commits_by_type.entry(c.commit_type).or_insert(0) += 1;
    }
    stats
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_feat() {
        let c = parse_commit("feat: add new search mode");
        assert_eq!(c.commit_type, CommitType::Feature);
        assert_eq!(c.description, "add new search mode");
        assert!(!c.breaking);
        assert!(c.scope.is_none());
    }

    #[test]
    fn test_parse_scoped_fix() {
        let c = parse_commit("fix(parser): handle empty input");
        assert_eq!(c.commit_type, CommitType::Fix);
        assert_eq!(c.scope.as_deref(), Some("parser"));
        assert_eq!(c.description, "handle empty input");
    }

    #[test]
    fn test_parse_breaking_bang() {
        let c = parse_commit("feat!: remove deprecated API");
        assert!(c.breaking);
        assert_eq!(c.commit_type, CommitType::Feature);
    }

    #[test]
    fn test_parse_breaking_footer() {
        let msg = "feat: new API\n\nBREAKING CHANGE: old API removed";
        let c = parse_commit(msg);
        assert!(c.breaking);
        assert_eq!(c.footers["BREAKING CHANGE"], "old API removed");
    }

    #[test]
    fn test_parse_non_conventional() {
        let c = parse_commit("random commit message");
        assert_eq!(c.commit_type, CommitType::Other);
        assert_eq!(c.description, "random commit message");
    }

    #[test]
    fn test_parse_with_body() {
        let msg = "docs: update README\n\nAdded new sections for v2.\nIncludes migration guide.";
        let c = parse_commit(msg);
        assert_eq!(c.commit_type, CommitType::Docs);
        assert!(c.body.is_some());
        assert!(c.body.as_ref().unwrap().contains("migration guide"));
    }

    #[test]
    fn test_semver_parse() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(v, SemVer::new(1, 2, 3));
        let v2 = SemVer::parse("v0.1.0-beta").unwrap();
        assert_eq!(v2.pre.as_deref(), Some("beta"));
    }

    #[test]
    fn test_semver_bumps() {
        let v = SemVer::new(1, 2, 3);
        assert_eq!(v.bump_patch(), SemVer::new(1, 2, 4));
        assert_eq!(v.bump_minor(), SemVer::new(1, 3, 0));
        assert_eq!(v.bump_major(), SemVer::new(2, 0, 0));
    }

    #[test]
    fn test_suggest_bump_major() {
        let commits = parse_commits(&["feat!: break API", "fix: typo"]);
        assert_eq!(suggest_bump(&commits), BumpKind::Major);
    }

    #[test]
    fn test_suggest_bump_minor() {
        let commits = parse_commits(&["feat: new feature", "fix: bug"]);
        assert_eq!(suggest_bump(&commits), BumpKind::Minor);
    }

    #[test]
    fn test_suggest_bump_patch() {
        let commits = parse_commits(&["fix: bug fix"]);
        assert_eq!(suggest_bump(&commits), BumpKind::Patch);
    }

    #[test]
    fn test_suggest_bump_none() {
        let commits = parse_commits(&["docs: update readme", "chore: cleanup"]);
        assert_eq!(suggest_bump(&commits), BumpKind::None);
    }

    #[test]
    fn test_generate_changelog_markdown() {
        let commits = parse_commits(&[
            "feat(core): add pattern registry",
            "fix(parser): handle edge case",
            "feat!: remove old API",
        ]);
        let entry = generate_changelog("2.0.0", Some("2026-03-06"), &commits);
        let md = render_markdown(&entry);
        assert!(md.contains("## [2.0.0]"));
        assert!(md.contains("BREAKING CHANGES"));
        assert!(md.contains("Features"));
        assert!(md.contains("Bug Fixes"));
        assert!(md.contains("**core**: add pattern registry"));
    }

    #[test]
    fn test_changelog_json() {
        let commits = parse_commits(&["feat: something"]);
        let entry = generate_changelog("1.0.0", None, &commits);
        let json = render_json(&entry);
        assert!(json.contains("\"version\""));
        assert!(json.contains("something"));
    }

    #[test]
    fn test_changelog_stats() {
        let commits = parse_commits(&["feat: a", "fix: b", "feat!: c", "random message"]);
        let stats = changelog_stats(&commits);
        assert_eq!(stats.total_commits, 4);
        assert_eq!(stats.conventional_commits, 3);
        assert_eq!(stats.breaking_changes, 1);
        assert_eq!(stats.commits_by_type[&CommitType::Feature], 2);
    }

    #[test]
    fn test_apply_bump() {
        let v = SemVer::new(1, 5, 0);
        assert_eq!(apply_bump(&v, BumpKind::Minor), SemVer::new(1, 6, 0));
        assert_eq!(apply_bump(&v, BumpKind::None), v);
    }
}
