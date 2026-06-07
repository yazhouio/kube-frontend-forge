use indexmap::{IndexMap, IndexSet};

#[derive(Clone, Default)]
pub struct ImportRegistry {
    namespace: IndexMap<String, String>,
    named: IndexMap<String, IndexSet<String>>,
    raw: IndexSet<String>,
}

impl ImportRegistry {
    pub fn add_source(&mut self, source: &str) {
        let source = normalize_import_source(source);
        if source.is_empty() {
            return;
        }

        if let Some((local, module)) = parse_namespace_import(&source) {
            self.add_namespace(module, local);
            return;
        }

        if let Some((names, module)) = parse_named_import(&source) {
            for name in names {
                self.add_named(module, name);
            }
            return;
        }

        self.raw.insert(format!("{source};"));
    }

    pub fn add_named(&mut self, module: impl Into<String>, name: impl Into<String>) {
        self.named
            .entry(module.into())
            .or_default()
            .insert(name.into());
    }

    pub fn add_namespace(&mut self, module: impl Into<String>, local: impl Into<String>) {
        let module = module.into();
        let local = local.into();
        if self
            .namespace
            .get(&module)
            .is_some_and(|existing| existing != &local)
        {
            self.raw
                .insert(format!("import * as {local} from \"{module}\";"));
            return;
        }
        self.namespace.insert(module, local);
    }

    pub fn has_named(&self, module: &str, name: &str) -> bool {
        self.named.get(module).is_some_and(|names| {
            names
                .iter()
                .any(|item| item == name || item.starts_with(&format!("{name} as ")))
        }) || self.raw.iter().any(|source| {
            source.contains(&format!("from \"{module}\""))
                && (source.contains(&format!("{{ {name} }}"))
                    || source.contains(&format!("{{ {name},"))
                    || source.contains(&format!(", {name},"))
                    || source.contains(&format!(", {name} }}")))
        })
    }

    pub fn emit_sources(&self) -> Vec<String> {
        let mut sources = Vec::new();
        for (module, local) in &self.namespace {
            sources.push(format!("import * as {local} from \"{module}\";"));
        }
        for (module, names) in &self.named {
            sources.push(format!(
                "import {{ {} }} from \"{module}\";",
                names.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        sources.extend(self.raw.iter().cloned());
        sources
    }
}

fn normalize_import_source(source: &str) -> String {
    source.trim().trim_end_matches(';').trim().to_owned()
}

fn parse_namespace_import(source: &str) -> Option<(&str, &str)> {
    let rest = source.strip_prefix("import * as ")?;
    let (local, module) = rest.split_once(" from ")?;
    parse_module_literal(module).map(|module| (local.trim(), module))
}

fn parse_named_import(source: &str) -> Option<(Vec<&str>, &str)> {
    let rest = source.strip_prefix("import {")?;
    let (names, module) = rest.split_once("} from ")?;
    let module = parse_module_literal(module)?;
    let names = names
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    Some((names, module))
}

fn parse_module_literal(source: &str) -> Option<&str> {
    source
        .trim()
        .strip_prefix('"')?
        .strip_suffix('"')
        .or_else(|| source.trim().strip_prefix('\'')?.strip_suffix('\''))
}

#[cfg(test)]
mod tests {
    use super::ImportRegistry;

    #[test]
    fn merges_named_imports_by_module() {
        let mut imports = ImportRegistry::default();
        imports.add_source(r#"import * as React from "react""#);
        imports.add_source(r#"import { useMemo } from "react";"#);
        imports.add_source(r#"import { useState } from "react""#);
        imports.add_source(r#"import { PageTable } from "@frontend-forge/forge-components";"#);
        imports.add_named("@frontend-forge/forge-components", "useRuntimeContext");

        let sources = imports.emit_sources();
        assert_eq!(sources[0], r#"import * as React from "react";"#);
        assert!(sources.contains(&r#"import { useMemo, useState } from "react";"#.to_owned()));
        assert!(sources.contains(
            &r#"import { PageTable, useRuntimeContext } from "@frontend-forge/forge-components";"#
                .to_owned()
        ));
    }
}
