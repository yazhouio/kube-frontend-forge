use indexmap::IndexSet;

use crate::error::{Error, Result};
use crate::value::pascal_case;

#[derive(Default)]
pub struct NameAllocator {
    used: IndexSet<String>,
}

impl NameAllocator {
    pub fn reserve(&mut self, name: impl Into<String>) {
        self.used.insert(name.into());
    }

    pub fn allocate(&mut self, preferred: &str, fallback: &str) -> Result<String> {
        let base = sanitize_ident(preferred).or_else(|_| sanitize_ident(fallback))?;
        self.allocate_ident(&base)
    }

    pub fn allocate_component_name(
        &mut self,
        preferred_name: Option<&str>,
        meta_title: Option<&str>,
        node_id: &str,
        node_type: &str,
    ) -> Result<String> {
        let base = preferred_name
            .map(|name| pascal_case(name, "Page"))
            .or_else(|| meta_title.map(|name| pascal_case(name, "Node")))
            .unwrap_or_else(|| pascal_case(node_id, &pascal_case(node_type, "Node")));
        self.allocate(&base, "Node")
    }

    fn allocate_ident(&mut self, base: &str) -> Result<String> {
        if base.is_empty() {
            return Err(Error::EmptyComponentName);
        }
        let mut name = base.to_owned();
        let mut index = 2;
        while self.used.contains(&name) {
            name = format!("{base}{index}");
            index += 1;
        }
        self.used.insert(name.clone());
        Ok(name)
    }
}

pub fn sanitize_ident(value: &str) -> Result<String> {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        let valid = ch == '_' || ch == '$' || ch.is_ascii_alphanumeric();
        if !valid {
            continue;
        }
        if index == 0 && ch.is_ascii_digit() {
            out.push('_');
        }
        out.push(ch);
    }
    if out.is_empty() {
        return Err(Error::EmptyComponentName);
    }
    Ok(out)
}
