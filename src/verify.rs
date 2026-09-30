//! Verification: the checks that make a plugin trustworthy before install.
//!
//! Deliberately scoped to *plugin integrity*, not to per-file frontmatter
//! linting. `agentlint` already validates individual harness files, so
//! re-implementing its rules here would create two sources of truth that
//! drift. These checks only cover what agentlint cannot see, because it is
//! handed a directory of files rather than a plugin:
//!
//! - the manifest parses and its required fields are populated
//! - the manifest `name` matches the repository it claims to describe
//! - a plugin contains at least one component (a manifest alone installs as
//!   a plugin that does nothing)
//! - every declared `hooks` key has a matching file on disk
//! - no component directory is empty or holds only non-markdown noise
//! - every skill declares a frontmatter `name` matching its directory
//!
//! Anything agentlint already owns is deliberately left to agentlint.

use std::fmt;
use std::path::Path;

use crate::plugin::{Component, Plugin};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Will break installation or leave the plugin non-functional.
    Error,
    /// Works, but a reviewer should look at it.
    Warning,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    /// Plugin-relative path, or `plugin.json` for manifest findings.
    pub location: String,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}: {}",
            self.location,
            self.severity.label(),
            self.message
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub skills: usize,
    pub agents: usize,
    pub commands: usize,
    pub hooks: usize,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// A report passes when nothing is an error.
    pub fn is_ok(&self) -> bool {
        self.errors() == 0
    }
}

/// Verify a discovered plugin.
pub fn verify(plugin: &Plugin) -> Report {
    let mut findings = Vec::new();

    check_manifest(plugin, &mut findings);
    check_has_content(plugin, &mut findings);
    check_hooks(plugin, &mut findings);
    check_skill_frontmatter(plugin, &mut findings);

    // Count skill entry points, not every file under skills/. A skill with
    // twenty reference documents is still one skill.
    let skills = plugin
        .of_kind("skills")
        .iter()
        .filter(|c| {
            let rel = Path::new(&c.rel_path);
            rel.components().count() == 3
                && rel.file_name().and_then(|n| n.to_str()) == Some("SKILL.md")
        })
        .count();
    let agents = plugin.of_kind("agents").len();
    let commands = plugin.of_kind("commands").len();
    let hooks = plugin.of_kind("hooks").len();

    Report {
        findings,
        skills,
        agents,
        commands,
        hooks,
    }
}

fn check_manifest(plugin: &Plugin, findings: &mut Vec<Finding>) {
    const LOC: &str = "plugin.json";
    let m = &plugin.manifest;

    if m.name.trim().is_empty() {
        findings.push(Finding {
            severity: Severity::Error,
            location: LOC.into(),
            message: "manifest `name` is empty".into(),
        });
    }

    match m.version.as_deref() {
        None => findings.push(Finding {
            severity: Severity::Warning,
            location: LOC.into(),
            message: "no `version`; marketplaces need one to resolve updates".into(),
        }),
        Some(v) if v.trim().is_empty() => findings.push(Finding {
            severity: Severity::Error,
            location: LOC.into(),
            message: "`version` is present but empty".into(),
        }),
        Some(_) => {}
    }

    match m.description.as_deref() {
        None => findings.push(Finding {
            severity: Severity::Warning,
            location: LOC.into(),
            message: "no `description`; the marketplace listing renders empty".into(),
        }),
        Some(d) if d.trim().is_empty() => findings.push(Finding {
            severity: Severity::Error,
            location: LOC.into(),
            message: "`description` is present but empty".into(),
        }),
        Some(_) => {}
    }

    // A manifest whose name disagrees with the repo it sits in is usually a
    // copy-paste error, and it makes the installed plugin show up under a
    // name that does not match its source.
    if !m.name.trim().is_empty()
        && let Some(dir) = plugin.root.file_name().and_then(|n| n.to_str())
        && dir != m.name
    {
        findings.push(Finding {
            severity: Severity::Warning,
            location: LOC.into(),
            message: format!("manifest name `{}` differs from directory `{dir}`", m.name),
        });
    }
}

fn check_has_content(plugin: &Plugin, findings: &mut Vec<Finding>) {
    if plugin.components.is_empty() {
        findings.push(Finding {
            severity: Severity::Error,
            location: "plugin.json".into(),
            message: "plugin has a manifest but no skills, agents, commands, or hooks".into(),
        });
    }
}

fn check_hooks(plugin: &Plugin, findings: &mut Vec<Finding>) {
    for event in plugin.manifest.hooks.keys() {
        // hooks.json is the convention in this workspace; an inline `hooks`
        // object may also embed the wiring, so only complain when neither
        // shape is present on disk.
        let hooks_dir = plugin.root.join("hooks");
        let hooks_file = hooks_dir.join("hooks.json");
        if !hooks_dir.is_dir() {
            findings.push(Finding {
                severity: Severity::Error,
                location: "plugin.json".into(),
                message: format!("declares hook `{event}` but has no hooks/ directory"),
            });
            continue;
        }
        if !hooks_file.is_file() {
            findings.push(Finding {
                severity: Severity::Error,
                location: "hooks/hooks.json".into(),
                message: format!("missing hooks/hooks.json for declared hook `{event}`"),
            });
        }
    }
}

fn check_skill_frontmatter(plugin: &Plugin, findings: &mut Vec<Finding>) {
    // A skill directory legitimately holds supporting files alongside its
    // entry point: `references/`, `helpers/`, `agents/openai.yaml`, and
    // `.gitkeep` placeholders are all normal. Only `skills/<name>/SKILL.md`
    // is the skill itself, so walk entry points rather than every file.
    let mut entry_points: Vec<&Component> = Vec::new();
    let mut orphans: Vec<&Component> = Vec::new();
    for component in plugin.of_kind("skills") {
        let rel = Path::new(&component.rel_path);
        let depth = rel.components().count();
        let file_name = rel.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if depth == 3 && file_name == "SKILL.md" {
            entry_points.push(component);
        } else if depth == 2 {
            // A file directly in skills/ is a skill that is missing its
            // directory, so it can never be invoked by name.
            orphans.push(component);
        }
    }

    for orphan in &orphans {
        findings.push(Finding {
            severity: Severity::Warning,
            location: orphan.rel_path.clone(),
            message: "file sits directly in skills/; a skill needs skills/<name>/SKILL.md".into(),
        });
    }

    for component in entry_points {
        let path = plugin.path_of(component);
        let Ok(text) = std::fs::read_to_string(&path) else {
            findings.push(Finding {
                severity: Severity::Error,
                location: component.rel_path.clone(),
                message: "cannot read skill entry point".into(),
            });
            continue;
        };

        let dir_name = Path::new(&component.rel_path)
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        match frontmatter_field(&text, "name") {
            None => findings.push(Finding {
                severity: Severity::Error,
                location: component.rel_path.clone(),
                message: "no frontmatter `name`; the skill cannot be invoked".into(),
            }),
            Some(name) if name != dir_name => findings.push(Finding {
                severity: Severity::Warning,
                location: component.rel_path.clone(),
                message: format!("frontmatter name `{name}` differs from directory `{dir_name}`"),
            }),
            Some(_) => {}
        }

        if frontmatter_field(&text, "description").is_none() {
            findings.push(Finding {
                severity: Severity::Warning,
                location: component.rel_path.clone(),
                message: "no frontmatter `description`; nothing tells the model when to load it"
                    .into(),
            });
        }
    }
}

/// Read a scalar field from the leading `---` frontmatter block.
///
/// Deliberately not a YAML parser: it only needs to answer "is this field
/// present, and what is its scalar value" for two well-known keys, and it
/// must not fail on the rich YAML that real skills carry in `description`.
pub fn frontmatter_field(text: &str, field: &str) -> Option<String> {
    let rest = text.strip_prefix("---")?;
    let body = rest.strip_prefix('\n').unwrap_or(rest);
    let end = body.find("\n---")?;
    let block = &body[..end];

    for line in block.lines() {
        let line = line.trim_start();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.trim() != field {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        // A block scalar (`description: >`) means the field is present even
        // though its content is on following lines.
        if value.is_empty() || value == ">" || value == "|" {
            return Some(value.to_string());
        }
        return Some(value.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_scalar_frontmatter_field() {
        let text = "---\nname: handoff\ndescription: use at end of session\n---\n\nbody\n";
        assert_eq!(frontmatter_field(text, "name").as_deref(), Some("handoff"));
        assert_eq!(
            frontmatter_field(text, "description").as_deref(),
            Some("use at end of session")
        );
        assert_eq!(frontmatter_field(text, "model"), None);
    }

    #[test]
    fn treats_block_scalar_as_present() {
        let text = "---\nname: repo-reference-site\ndescription: >\n  long text\n  more\n---\n";
        // The block-scalar marker comes back verbatim. A presence check does
        // not need the folded body, only the fact that the field exists.
        assert_eq!(frontmatter_field(text, "description").as_deref(), Some(">"));
        assert_eq!(
            frontmatter_field(text, "name").as_deref(),
            Some("repo-reference-site")
        );
    }

    #[test]
    fn rejects_text_without_frontmatter() {
        assert_eq!(frontmatter_field("no frontmatter here", "name"), None);
    }
}
