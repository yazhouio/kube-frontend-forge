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
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_owned());
    let command_args = args.collect::<Vec<_>>();
    match command.as_str() {
        "generate-schemas" => generate_schemas(),
        "check-schemas" => check_schemas(),
        "fmt" => fmt(&command_args),
        "clippy" => clippy(&command_args),
        "commit-msg" => commit_msg(&command_args),
        "verify" => verify(),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => Err(format!("unknown xtask command `{command}`").into()),
    }
}

fn print_help() {
    println!(
        "usage: cargo run -p xtask -- <generate-schemas|check-schemas|fmt [--check]|clippy|commit-msg <path>|verify>"
    );
}

fn verify() -> Result<()> {
    check_schemas()?;
    run_fmt(true)?;
    run_clippy()
}

fn fmt(args: &[String]) -> Result<()> {
    match args {
        [] => run_fmt(false),
        [arg] if arg == "--check" => run_fmt(true),
        _ => Err("usage: cargo run -p xtask -- fmt [--check]".into()),
    }
}

fn clippy(args: &[String]) -> Result<()> {
    if !args.is_empty() {
        return Err("usage: cargo run -p xtask -- clippy".into());
    }
    run_clippy()
}

fn commit_msg(args: &[String]) -> Result<()> {
    let [path] = args else {
        return Err("usage: cargo run -p xtask -- commit-msg <path>".into());
    };
    let content = fs::read_to_string(path)?;
    validate_commit_message(&content).map_err(Into::into)
}

fn validate_commit_message(message: &str) -> std::result::Result<(), String> {
    let message = clean_commit_message(message);
    if message.is_empty() {
        return Err("commit message cannot be empty".to_owned());
    }

    let Some(subject) = commit_subject(&message) else {
        return Err("commit message cannot be empty".to_owned());
    };

    if is_git_generated_subject(subject) {
        return Ok(());
    }

    git_conventional::Commit::parse(&message)
        .map(|_| ())
        .map_err(|source| {
            format!(
                "invalid Conventional Commit message `{subject}`: {source}; expected format like `feat: add static file server`"
            )
        })
}

fn clean_commit_message(message: &str) -> String {
    let lines = message
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return String::new();
    };
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .expect("start exists");
    lines[start..=end].join("\n")
}

fn commit_subject(message: &str) -> Option<&str> {
    message
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
}

fn is_git_generated_subject(subject: &str) -> bool {
    subject.starts_with("Merge ")
        || subject.starts_with("Revert ")
        || subject.starts_with("fixup! ")
        || subject.starts_with("squash! ")
        || subject == "Initial commit"
}

fn run_fmt(check: bool) -> Result<()> {
    if check {
        run_cargo(&["fmt", "--all", "--", "--check"])
    } else {
        run_cargo(&["fmt", "--all"])
    }
}

fn run_clippy() -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::validate_commit_message;

    #[test]
    fn accepts_conventional_commit_subjects() {
        validate_commit_message("feat: add static file server\n").expect("feat should pass");
        validate_commit_message("refactor(core)!: split build plan\n")
            .expect("scoped breaking change should pass");
        validate_commit_message(
            "feat: add static file server\n\nBREAKING CHANGE: server config changed\n",
        )
        .expect("breaking footer should pass");
    }

    #[test]
    fn accepts_git_generated_subjects() {
        validate_commit_message("Merge branch 'main' into feature\n").expect("merge should pass");
        validate_commit_message("fixup! feat: add static file server\n")
            .expect("fixup should pass");
    }

    #[test]
    fn ignores_commit_template_comments() {
        validate_commit_message("\n# comment\nfeat: add static file server\n")
            .expect("comments should be ignored");
    }

    #[test]
    fn rejects_invalid_subjects() {
        assert!(validate_commit_message("").is_err());
        assert!(validate_commit_message("wip\n").is_err());
        assert!(validate_commit_message("Add static file config to forge server\n").is_err());
        assert!(validate_commit_message("添加静态文件服务\n").is_err());
        assert!(validate_commit_message("feat:\n").is_err());
    }
}
