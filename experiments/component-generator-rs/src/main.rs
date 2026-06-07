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
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or(Error::MissingInputPath)?;
    let mut backend = "oxc".to_owned();
    while let Some(arg) = args.next() {
        if arg == "--backend" {
            backend = args.next().ok_or(Error::MissingInputPath)?;
        }
    }

    let raw = std::fs::read_to_string(&path).map_err(|source| Error::ReadFile {
        path: path.clone(),
        source,
    })?;
    let value = serde_json::from_str(&raw).map_err(|source| Error::ParseJson {
        path: path.clone(),
        source,
    })?;
    let page = unwrap_page_schema(value)?;
    let code = match backend.as_str() {
        "oxc" => ComponentGenerator::with_backend(default_registry(), OxcCodeBackend)
            .generate_page_code(&page)?,
        #[cfg(feature = "swc")]
        "swc" => ComponentGenerator::with_backend(default_registry(), SwcCodeBackend)
            .generate_page_code(&page)?,
        #[cfg(not(feature = "swc"))]
        "swc" => {
            return Err(Error::BackendFeatureDisabled {
                backend: backend.clone(),
                feature: "swc".to_owned(),
            });
        }
        _ => {
            return Err(Error::InvalidBackend {
                backend: backend.clone(),
            });
        }
    };
    println!("{code}");
    Ok(())
}
