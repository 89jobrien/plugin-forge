//! Scaffolding: create a plugin skeleton on disk.
//!
//! Emits the same layout the existing plugins in this workspace use, so a
//! generated plugin is installable and verifiable without further edits.

use std::path::{Path, PathBuf};

use crate::plugin::{Author, Manifest, Plugin, PluginError};

/// Files a scaffolded plugin starts with, in creation order.
#[derive(Debug, Clone)]
pub struct Scaffold {
    pub root: PathBuf,
    pub name: String,
    pub description: String,
    pub version: String,
}

impl Scaffold {
    pub fn new(root: &Path, name: &str, description: &str) -> Self {
        Self {
            root: root.join(name),
            name: name.to_string(),
            description: description.to_string(),
            version: "0.1.0".to_string(),
        }
    }

    /// Create the directory tree, manifest, and a first skill.
    ///
    /// Refuses to overwrite an existing directory: scaffolding over a real
    /// plugin would silently discard work.
    pub fn write(&self) -> Result<Vec<PathBuf>, PluginError> {
        if self.root.exists() {
            return Err(PluginError::Scaffold(format!(
                "{} already exists; refusing to overwrite",
                self.root.display()
            )));
        }

        let manifest = Manifest {
            name: self.name.clone(),
            version: Some(self.version.clone()),
            description: Some(self.description.clone()),
            author: Some(Author {
                name: Some("Joseph O'Brien".into()),
                email: None,
            }),
            keywords: Vec::new(),
            hooks: Default::default(),
        };

        let mut written = Vec::new();
        let manifest_dir = self.root.join(".claude-plugin");
        create_dir(&manifest_dir)?;
        written.push(write(
            &manifest_dir.join("plugin.json"),
            &serde_json::to_string_pretty(&manifest)?,
        )?);

        let skill_dir = self.root.join("skills").join(&self.name);
        create_dir(&skill_dir)?;
        written.push(write(
            &skill_dir.join("SKILL.md"),
            &skill_template(&self.name, &self.description),
        )?);

        create_dir(&self.root.join("agents"))?;
        create_dir(&self.root.join("commands"))?;
        create_dir(&self.root.join("hooks"))?;

        written.push(write(
            &self.root.join("README.md"),
            &readme_template(&self.name, &self.description),
        )?);

        Ok(written)
    }
}

fn create_dir(path: &Path) -> Result<(), PluginError> {
    std::fs::create_dir_all(path).map_err(|e| PluginError::Create(path.to_path_buf(), e))
}

fn write(path: &Path, contents: &str) -> Result<PathBuf, PluginError> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    std::fs::write(path, contents).map_err(|e| PluginError::Create(path.to_path_buf(), e))?;
    Ok(path.to_path_buf())
}

fn skill_template(name: &str, description: &str) -> String {
    format!(
        "---\n\
         name: {name}\n\
         description: >\n\
         \x20 {description}\n\
         ---\n\
         \n\
         # {name}\n\
         \n\
         Describe when this skill applies and the steps it performs. Keep the\n\
         description trigger-oriented: it is what the model reads to decide\n\
         whether to load this file at all.\n\
         \n\
         ## Steps\n\
         \n\
         1. Replace this with the first step.\n"
    )
}

fn readme_template(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\
         \n\
         {description}\n\
         \n\
         ## Layout\n\
         \n\
         ```text\n\
         {name}/\n\
         ├── .claude-plugin/plugin.json\n\
         ├── skills/{name}/SKILL.md\n\
         ├── agents/\n\
         ├── commands/\n\
         └── hooks/\n\
         ```\n\
         \n\
         ## Verify\n\
         \n\
         ```bash\n\
         plugin-forge verify {name}\n\
         ```\n\
         \n\
         `verify` checks plugin integrity. For per-file frontmatter linting of\n\
         every harness file, use `agentlint`.\n"
    )
}

/// Verify a path, returning `None` when it is not a plugin at all.
pub fn discover_or_none(path: &Path) -> Result<Option<Plugin>, PluginError> {
    Plugin::discover(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_then_discover_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let scaffold = Scaffold::new(tmp.path(), "demo", "A demo plugin");
        let written = scaffold.write().unwrap();
        assert!(!written.is_empty());
        assert!(scaffold.root.join(".claude-plugin/plugin.json").is_file());

        let plugin = Plugin::discover(&scaffold.root)
            .unwrap()
            .expect("is a plugin");
        assert_eq!(plugin.manifest.name, "demo");
        assert_eq!(plugin.manifest.version.as_deref(), Some("0.1.0"));
        assert_eq!(plugin.of_kind("skills").len(), 1);

        let report = crate::verify::verify(&plugin);
        assert!(
            report.is_ok(),
            "fresh scaffold must verify clean: {report:?}"
        );
    }

    #[test]
    fn scaffold_refuses_to_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let scaffold = Scaffold::new(tmp.path(), "demo", "A demo plugin");
        scaffold.write().unwrap();
        let again = Scaffold::new(tmp.path(), "demo", "A demo plugin");
        assert!(again.write().is_err());
    }

    #[test]
    fn discover_returns_none_for_non_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(Plugin::discover(tmp.path()).unwrap().is_none());
    }
}
