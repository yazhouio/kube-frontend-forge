use forge_project_generator::{
    Error, GenerateProjectFilesOptions, Result, generate_project_files, unwrap_manifest,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = parse_args(std::env::args().skip(1))?;
    let raw = std::fs::read_to_string(&args.path).map_err(|source| Error::ReadFile {
        path: args.path.clone(),
        source,
    })?;
    let value = serde_json::from_str(&raw).map_err(|source| Error::ParseJson {
        path: args.path.clone(),
        source,
    })?;
    let manifest = unwrap_manifest(value)?;
    let result = generate_project_files(
        &manifest,
        |page, _manifest| {
            Ok(format!(
                "export default function {}() {{\n  return null;\n}}\n",
                page.entry_component
            ))
        },
        GenerateProjectFilesOptions {
            build: args.build,
            archive: args.archive,
        },
    )?;
    let output =
        serde_json::to_string_pretty(&result).map_err(|source| Error::SerializeJson { source })?;

    if let Some(out_path) = args.out_path {
        std::fs::write(&out_path, output).map_err(|source| Error::WriteFile {
            path: out_path,
            source,
        })?;
    } else {
        println!("{output}");
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct CliArgs {
    path: String,
    out_path: Option<String>,
    build: bool,
    archive: bool,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<CliArgs> {
    let mut path = None;
    let mut out_path = None;
    let mut build = false;
    let mut archive = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                out_path = Some(args.next().ok_or(Error::MissingOutPath)?);
            }
            "--build" => {
                build = true;
            }
            "--archive" => {
                archive = true;
            }
            _ if arg.starts_with('-') => {
                return Err(Error::UnknownArgument { arg });
            }
            _ => {
                if path.replace(arg.clone()).is_some() {
                    return Err(Error::UnexpectedInputPath { path: arg });
                }
            }
        }
    }

    Ok(CliArgs {
        path: path.ok_or(Error::MissingInputPath)?,
        out_path,
        build,
        archive,
    })
}

#[cfg(test)]
mod tests {
    use super::{CliArgs, parse_args};

    #[test]
    fn parse_args_accepts_input_path() {
        let args = parse_args(["manifest.json".to_owned()]).unwrap();

        assert_eq!(
            args,
            CliArgs {
                path: "manifest.json".to_owned(),
                out_path: None,
                build: false,
                archive: false,
            }
        );
    }

    #[test]
    fn parse_args_accepts_flags() {
        let args = parse_args([
            "manifest.json".to_owned(),
            "--build".to_owned(),
            "--archive".to_owned(),
            "--out".to_owned(),
            "result.json".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            args,
            CliArgs {
                path: "manifest.json".to_owned(),
                out_path: Some("result.json".to_owned()),
                build: true,
                archive: true,
            }
        );
    }

    #[test]
    fn parse_args_rejects_missing_out_path() {
        let error = parse_args(["manifest.json".to_owned(), "--out".to_owned()])
            .unwrap_err()
            .to_string();

        assert_eq!(error, "missing value for --out");
    }
}
