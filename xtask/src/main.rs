use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use forge_core::ForgeCore;
use serde_json::Value;

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "help".to_owned());
    match command.as_str() {
        "generate-schemas" => generate_schemas(),
        "check-schemas" => check_schemas(),
        "verify" => verify(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => Err(format!("unknown xtask command `{command}`").into()),
    }
}

fn print_help() {
    println!("usage: cargo run -p xtask -- <generate-schemas|check-schemas|verify>");
}

fn verify() -> Result<()> {
    check_schemas()?;
    run_cargo(&["fmt", "--all", "--", "--check"])?;
    run_cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])
}

fn generate_schemas() -> Result<()> {
    let root = repo_root();
    let schemas_dir = root.join("schemas");
    fs::create_dir_all(&schemas_dir)?;
    for (name, schema) in generated_schemas()? {
        fs::write(schemas_dir.join(name), schema_content(&schema)?)?;
    }
    Ok(())
}

fn check_schemas() -> Result<()> {
    let root = repo_root();
    let schemas_dir = root.join("schemas");
    let mut stale = Vec::new();

    for (name, schema) in generated_schemas()? {
        let path = schemas_dir.join(name);
        let expected = schema_content(&schema)?;
        let actual = fs::read_to_string(&path).unwrap_or_default();
        if actual != expected {
            stale.push(path);
        }
    }

    if stale.is_empty() {
        return Ok(());
    }

    let files = stale
        .iter()
        .map(|path| display_path(path))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "generated schema files are not up to date: {files}; run `cargo run -p xtask -- generate-schemas`"
    )
    .into())
}

fn generated_schemas() -> Result<Vec<(&'static str, Value)>> {
    let core = ForgeCore::try_new()?;
    Ok(vec![
        ("manifest.schema.json", core.manifest_schema()),
        ("component-tree.schema.json", core.component_tree_schema()),
        ("node-source.schema.json", core.node_source_schema()),
        (
            "data-source-source.schema.json",
            core.data_source_source_schema(),
        ),
    ])
}

fn schema_content(schema: &Value) -> Result<String> {
    Ok(serde_json::to_string_pretty(schema)? + "\n")
}

fn run_cargo(args: &[&str]) -> Result<()> {
    let status = Command::new("cargo")
        .args(args)
        .current_dir(repo_root())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed with {status}", args.join(" ")).into())
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives under repo root")
        .to_path_buf()
}

fn display_path(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
