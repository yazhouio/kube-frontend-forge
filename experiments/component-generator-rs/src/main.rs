#[cfg(feature = "swc")]
use component_generator_rs::SwcCodeBackend;
use component_generator_rs::{
    ComponentGenerator, Error, OxcCodeBackend, Result, builtins::default_registry,
    unwrap_page_schema,
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
    let page = unwrap_page_schema(value)?;
    let code = match args.backend.as_str() {
        "oxc" => ComponentGenerator::with_backend(default_registry(), OxcCodeBackend)
            .generate_page_code(&page)?,
        #[cfg(feature = "swc")]
        "swc" => ComponentGenerator::with_backend(default_registry(), SwcCodeBackend)
            .generate_page_code(&page)?,
        #[cfg(not(feature = "swc"))]
        "swc" => {
            return Err(Error::BackendFeatureDisabled {
                backend: args.backend.clone(),
                feature: "swc".to_owned(),
            });
        }
        _ => {
            return Err(Error::InvalidBackend {
                backend: args.backend.clone(),
            });
        }
    };
    if let Some(out_path) = args.out_path {
        std::fs::write(&out_path, code).map_err(|source| Error::WriteFile {
            path: out_path,
            source,
        })?;
    } else {
        println!("{code}");
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct CliArgs {
    path: String,
    backend: String,
    out_path: Option<String>,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<CliArgs> {
    let mut path = None;
    let mut backend = "oxc".to_owned();
    let mut out_path = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--backend" => {
                backend = args.next().ok_or(Error::MissingBackendValue)?;
            }
            "--out" => {
                out_path = Some(args.next().ok_or(Error::MissingOutPath)?);
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
        backend,
        out_path,
    })
}

#[cfg(test)]
mod tests {
    use super::{CliArgs, parse_args};

    #[test]
    fn parse_args_defaults_to_oxc() {
        let args = parse_args(["page.json".to_owned()]).unwrap();

        assert_eq!(
            args,
            CliArgs {
                path: "page.json".to_owned(),
                backend: "oxc".to_owned(),
                out_path: None,
            }
        );
    }

    #[test]
    fn parse_args_accepts_backend_flag() {
        let args = parse_args([
            "page.json".to_owned(),
            "--backend".to_owned(),
            "swc".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            args,
            CliArgs {
                path: "page.json".to_owned(),
                backend: "swc".to_owned(),
                out_path: None,
            }
        );
    }

    #[test]
    fn parse_args_accepts_out_flag() {
        let args = parse_args([
            "page.json".to_owned(),
            "--out".to_owned(),
            "page.tsx".to_owned(),
        ])
        .unwrap();

        assert_eq!(
            args,
            CliArgs {
                path: "page.json".to_owned(),
                backend: "oxc".to_owned(),
                out_path: Some("page.tsx".to_owned()),
            }
        );
    }

    #[test]
    fn parse_args_rejects_missing_backend_value() {
        let error = parse_args(["page.json".to_owned(), "--backend".to_owned()])
            .unwrap_err()
            .to_string();

        assert_eq!(
            error,
            "missing value for --backend; expected `oxc` or `swc`"
        );
    }

    #[test]
    fn parse_args_rejects_missing_out_path() {
        let error = parse_args(["page.json".to_owned(), "--out".to_owned()])
            .unwrap_err()
            .to_string();

        assert_eq!(error, "missing value for --out");
    }

    #[test]
    fn parse_args_rejects_unknown_flags() {
        let error = parse_args(["page.json".to_owned(), "--format".to_owned()])
            .unwrap_err()
            .to_string();

        assert_eq!(error, "unknown argument `--format`");
    }

    #[test]
    fn parse_args_rejects_extra_input_path() {
        let error = parse_args(["page.json".to_owned(), "other.json".to_owned()])
            .unwrap_err()
            .to_string();

        assert_eq!(error, "unexpected extra input path `other.json`");
    }
}
