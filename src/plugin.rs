//! Plugin domain model: manifest, discovery, and component inventory.
//!
//! The layout mirrors the Claude Code plugin format already in use by
//! atelier, sanctum, and orca-strait:
//!
//! ```text
//! <root>/
//! ├── .claude-plugin/plugin.json
//! ├── skills/<name>/SKILL.md
//! ├── agents/<name>.md
//! ├── commands/<name>.md
//! └── hooks/hooks.json
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A plugin manifest as written to `.claude-plugin/plugin.json`.
///
/// Every field beyond `name` is optional, because real plugins in this
/// workspace omit them: orca-strait carries `keywords` and `hooks`, sanctum
/// carries neither. Verification reports the gaps rather than refusing to
/// parse them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Author>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hooks: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// One file belonging to a plugin, classified by the directory it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// Directory class: `skills`, `agents`, `commands`, `hooks`, or `other`.
    pub kind: String,
    /// Path relative to the plugin root, using forward slashes.
    pub rel_path: String,
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.rel_path)
    }
}

/// A discovered plugin: its root, parsed manifest, and component inventory.
#[derive(Debug, Clone)]
pub struct Plugin {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub components: Vec<Component>,
}

impl Plugin {
    /// Locate the plugin rooted at `root` by finding its manifest.
    ///
    /// Returns `Ok(None)` when the directory is not a plugin, which is the
    /// normal case when this runs across a workspace that contains both
    /// plugins and ordinary Rust crates.
    pub fn discover(root: &Path) -> Result<Option<Self>, PluginError> {
        let manifest_path = root.join(".claude-plugin").join("plugin.json");
        if !manifest_path.is_file() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&manifest_path)
            .map_err(|source| PluginError::Read(manifest_path.clone(), source))?;
        let manifest: Manifest = serde_json::from_str(&raw)
            .map_err(|source| PluginError::Manifest(manifest_path.clone(), source))?;
        let components = inventory(root)?;
        Ok(Some(Self {
            root: root.to_path_buf(),
            manifest,
            components,
        }))
    }

    /// Components of a given kind, in stable path order.
    pub fn of_kind(&self, kind: &str) -> Vec<&Component> {
        self.components.iter().filter(|c| c.kind == kind).collect()
    }

    /// Absolute path of a component's file on disk.
    pub fn path_of(&self, component: &Component) -> PathBuf {
        self.root.join(&component.rel_path)
    }
}

/// Walk `root` and classify every file that belongs to a known plugin
/// component directory.
pub fn inventory(root: &Path) -> Result<Vec<Component>, PluginError> {
    const KINDS: &[&str] = &["skills", "agents", "commands", "hooks"];
    let mut out = Vec::new();
    for kind in KINDS {
        let dir = root.join(kind);
        if !dir.is_dir() {
            continue;
        }
        walk(&dir, root, kind, &mut out)?;
    }
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(out)
}

fn walk(dir: &Path, root: &Path, kind: &str, out: &mut Vec<Component>) -> Result<(), PluginError> {
    let entries =
        std::fs::read_dir(dir).map_err(|source| PluginError::Read(dir.to_path_buf(), source))?;
    for entry in entries {
        let entry = entry.map_err(|source| PluginError::Read(dir.to_path_buf(), source))?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, kind, out)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
            out.push(Component {
                kind: (*kind).to_string(),
                rel_path: rel,
            });
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum PluginError {
    Read(PathBuf, std::io::Error),
    Manifest(PathBuf, serde_json::Error),
    Serialize(serde_json::Error),
    Create(PathBuf, std::io::Error),
    Scaffold(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(p, e) => write!(f, "cannot read {}: {e}", p.display()),
            Self::Manifest(p, e) => write!(f, "invalid plugin manifest {}: {e}", p.display()),
            Self::Serialize(e) => write!(f, "cannot serialize manifest: {e}"),
            Self::Create(p, e) => write!(f, "cannot create {}: {e}", p.display()),
            Self::Scaffold(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for PluginError {}

impl From<serde_json::Error> for PluginError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialize(e)
    }
}
