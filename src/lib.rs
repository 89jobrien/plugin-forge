//! Build and verify agent harness plugins.
//!
//! `plugin-forge` owns plugin *integrity*: the manifest, the component
//! inventory, and the relationships between them. It deliberately does not
//! re-lint individual harness files — `agentlint` owns that, and two
//! implementations of the same rules would drift apart.
//!
//! ```no_run
//! use plugin_forge::{Plugin, verify};
//!
//! let plugin = Plugin::discover(std::path::Path::new("./atelier"))?.unwrap();
//! let report = verify(&plugin);
//! assert!(report.is_ok());
//! # Ok::<(), plugin_forge::PluginError>(())
//! ```

pub mod plugin;
pub mod scaffold;
pub mod verify;

pub use plugin::{Author, Component, Manifest, Plugin, PluginError};
pub use scaffold::Scaffold;
pub use verify::{Finding, Report, Severity, verify};
