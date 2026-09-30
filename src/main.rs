//! CLI entry point: `plugin-forge new` and `plugin-forge verify`.
//!
//! The parser is hand-rolled rather than pulled from `clap`, matching the
//! neighbouring scaffolding tools in this workspace and keeping the
//! dependency surface to serde alone.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use plugin_forge::{Plugin, Scaffold};

const USAGE: &str = "\
plugin-forge — build and verify agent harness plugins

USAGE:
    plugin-forge new <name> [--description <text>] [--root <path>]
    plugin-forge verify <path> [--json]
    plugin-forge inventory <path>

COMMANDS:
    new       Scaffold a plugin skeleton. Refuses to overwrite.
    verify    Check plugin integrity; exit 1 on any error.
    inventory  List the manifest and component inventory.

NOTES:
    Per-file frontmatter linting of harness files is `agentlint`'s job.
    plugin-forge checks the plugin as a unit.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("plugin-forge: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some(command) = args.first() else {
        print!("{USAGE}");
        return Ok(ExitCode::FAILURE);
    };

    match command.as_str() {
        "new" => cmd_new(&args[1..]),
        "verify" => cmd_verify(&args[1..]),
        "inventory" => cmd_inventory(&args[1..]),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    }
}

fn cmd_new(args: &[String]) -> Result<ExitCode, String> {
    let mut name: Option<String> = None;
    let mut description = "Describe what this plugin does.".to_string();
    let mut root = PathBuf::from(".");

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--description" => {
                description = args
                    .get(i + 1)
                    .ok_or_else(|| "--description needs a value".to_string())?
                    .clone();
                i += 2;
            }
            "--root" => {
                let raw = args
                    .get(i + 1)
                    .ok_or_else(|| "--root needs a value".to_string())?;
                root = PathBuf::from(raw);
                i += 2;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}` for `new`"));
            }
            other => {
                if name.is_some() {
                    return Err(format!("unexpected extra argument `{other}`"));
                }
                name = Some(other.to_string());
                i += 1;
            }
        }
    }

    let name = name.ok_or_else(|| "`new` needs a plugin name".to_string())?;
    let scaffold = Scaffold::new(&root, &name, &description);
    let written = scaffold
        .write()
        .map_err(|e| format!("scaffold failed: {e}"))?;

    println!("created {name} at {}", scaffold.root.display());
    println!("files:");
    for path in &written {
        println!("  - {}", path.display());
    }
    println!("next steps:");
    println!("  - plugin-forge verify {name}");
    Ok(ExitCode::SUCCESS)
}

fn cmd_verify(args: &[String]) -> Result<ExitCode, String> {
    let json = args.iter().any(|a| a == "--json");
    let path = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .ok_or_else(|| "`verify` needs a plugin path".to_string())?;

    let plugin = load(path)?;
    let report = plugin_forge::verify::verify(&plugin);

    if json {
        let payload = serde_json::json!({
            "name": plugin.manifest.name,
            "root": plugin.root.display().to_string(),
            "skills": report.skills,
            "agents": report.agents,
            "commands": report.commands,
            "hooks": report.hooks,
            "errors": report.errors(),
            "warnings": report.warnings(),
            "findings": report.findings.iter().map(|f| serde_json::json!({
                "severity": f.severity.label(),
                "location": f.location,
                "message": f.message,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else {
        println!("=== {} ===", plugin.manifest.name);
        println!(
            "  components  {} skill(s), {} agent(s), {} command(s), {} hook file(s)",
            report.skills, report.agents, report.commands, report.hooks
        );
        for finding in &report.findings {
            println!("  {}", finding);
        }
        if report.is_ok() {
            println!("  PASS ({} warning(s))", report.warnings());
        } else {
            println!("  FAIL ({} error(s))", report.errors());
        }
    }

    Ok(if report.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn cmd_inventory(args: &[String]) -> Result<ExitCode, String> {
    let path = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .ok_or_else(|| "`inventory` needs a plugin path".to_string())?;
    let plugin = load(path)?;

    println!("plugin   {}", plugin.manifest.name);
    println!(
        "version  {}",
        plugin.manifest.version.as_deref().unwrap_or("-")
    );
    println!(
        "summary  {}",
        plugin.manifest.description.as_deref().unwrap_or("-")
    );
    for component in &plugin.components {
        println!("  {:<10} {}", component.kind, component.rel_path);
    }
    Ok(ExitCode::SUCCESS)
}

fn load(path: &str) -> Result<Plugin, String> {
    let root = Path::new(path);
    if !root.is_dir() {
        return Err(format!("{path} is not a directory"));
    }
    Plugin::discover(root)
        .map_err(|e| format!("{path}: {e}"))?
        .ok_or_else(|| format!("{path} is not a plugin (no .claude-plugin/plugin.json)"))
}
