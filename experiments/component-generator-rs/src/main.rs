use component_generator_rs::{ComponentGenerator, Error, Result, unwrap_page_schema};

fn main() -> Result<()> {
    let path = std::env::args().nth(1).ok_or(Error::MissingInputPath)?;
    let raw = std::fs::read_to_string(&path).map_err(|source| Error::ReadFile {
        path: path.clone(),
        source,
    })?;
    let value = serde_json::from_str(&raw).map_err(|source| Error::ParseJson {
        path: path.clone(),
        source,
    })?;
    let page = unwrap_page_schema(value)?;
    let code = ComponentGenerator::default().generate_page_code(&page)?;
    println!("{code}");
    Ok(())
}
