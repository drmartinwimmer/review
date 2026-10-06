use crate::extractors::cli_args::{extract_doc_comment, has_derive_attribute, to_kebab_case};
use crate::model::{ConfigApi, ConfigOptionItem, ConfigSectionItem};
use quote::ToTokens;
use std::fs;
use std::path::Path;
use syn::{Fields, File, Item, ItemStruct, Lit};

/// Extractor for configuration file formats (e.g. `Cargo.toml` tables) defined via `ConfigFile`.
#[derive(Debug, Default)]
pub(crate) struct ConfigExtractor;

impl ConfigExtractor {
    /// Creates a new `ConfigExtractor`.
    pub(crate) fn new() -> Self {
        Self
    }

    /// Extracts configuration format specifications from a crate root by dynamically inspecting source files.
    pub fn extract_from_crate(&self, crate_root: &Path) -> std::io::Result<Option<ConfigApi>> {
        let src_dir = crate_root.join("src");
        if !src_dir.exists() {
            return Ok(None);
        }

        let mut sections = Vec::new();
        self.scan_dir_for_config(&src_dir, &mut sections)?;

        if sections.is_empty() {
            Ok(None)
        } else {
            sections.sort_by(|a, b| a.section.cmp(&b.section));
            sections.dedup_by(|a, b| a.section == b.section);
            Ok(Some(ConfigApi::new(sections)))
        }
    }

    fn scan_dir_for_config(
        &self,
        dir: &Path,
        sections: &mut Vec<ConfigSectionItem>,
    ) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.scan_dir_for_config(&path, sections)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(file) = syn::parse_file(&content)
            {
                self.extract_from_file(&file, sections);
            }
        }
        Ok(())
    }

    pub(crate) fn extract_from_file(&self, file: &File, sections: &mut Vec<ConfigSectionItem>) {
        for item in &file.items {
            let Item::Struct(item_struct) = item else {
                continue;
            };

            if let Some(sec) = extract_config_section(item_struct) {
                sections.push(sec);
            }
        }
    }
}

fn extract_config_section(item_struct: &ItemStruct) -> Option<ConfigSectionItem> {
    let struct_name = item_struct.ident.to_string();

    if struct_name == "PuristLintsConfig" || struct_name == "OpinionatedLintsConfig" {
        return extract_lints_config_section(item_struct);
    }
    if struct_name == "MaxFileLinesConfig" {
        return extract_file_lines_config_section(item_struct);
    }

    if !is_config_struct(item_struct) {
        return None;
    }

    extract_dynamic_config_section(item_struct)
}

fn is_config_struct(item_struct: &ItemStruct) -> bool {
    has_derive_attribute(&item_struct.attrs, "ConfigFile")
        || has_config_file_attribute(&item_struct.attrs)
}

fn has_config_file_attribute(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.path().is_ident("config_file") || a.path().is_ident("config_section"))
}

fn extract_lints_config_section(item_struct: &ItemStruct) -> Option<ConfigSectionItem> {
    let Fields::Named(named) = &item_struct.fields else {
        return None;
    };

    let options: Vec<ConfigOptionItem> = named
        .named
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .map(|ident| {
            let key = ident.to_string();
            let doc = format!("Rule level for purist::{key}");
            ConfigOptionItem::new(
                key,
                "\"allow\" | \"warn\" | \"deny\" | \"forbid\"",
                Some("\"warn\"".to_string()),
                Some(doc),
            )
        })
        .collect();

    let doc = "Purist lint rule severity levels configured in Cargo.toml. Accepts 'allow', 'warn', 'deny', or 'forbid'.".to_string();
    Some(ConfigSectionItem::new(
        "[lints.purist]",
        "Cargo.toml",
        Some(doc),
        options,
    ))
}

fn extract_file_lines_config_section(item_struct: &ItemStruct) -> Option<ConfigSectionItem> {
    let Fields::Named(named) = &item_struct.fields else {
        return None;
    };

    let mut options = Vec::new();
    for field in &named.named {
        let Some(ident) = &field.ident else {
            continue;
        };
        let key = ident.to_string();
        let (default_val, doc_str) = match key.as_str() {
            "max_production_lines" => (
                Some("600".to_string()),
                "Maximum allowed production lines per source file",
            ),
            "max_total_lines" => (
                Some("1000".to_string()),
                "Maximum allowed total lines per file (including tests)",
            ),
            _ => (None, "Line limit threshold"),
        };
        options.push(ConfigOptionItem::new(
            key,
            "integer",
            default_val,
            Some(doc_str.to_string()),
        ));
    }

    let doc = "Per-package threshold configurations for Purist static analysis rules.".to_string();
    Some(ConfigSectionItem::new(
        "[package.metadata.purist]",
        "Cargo.toml",
        Some(doc),
        options,
    ))
}

fn extract_dynamic_config_section(item_struct: &ItemStruct) -> Option<ConfigSectionItem> {
    let Fields::Named(named) = &item_struct.fields else {
        return None;
    };

    let file_fmt =
        parse_config_file_path(&item_struct.attrs).unwrap_or_else(|| "Cargo.toml".to_string());
    let sec_name = parse_config_section_name(&item_struct.attrs).unwrap_or_else(|| {
        let kebab = to_kebab_case(&item_struct.ident.to_string());
        format!("[{kebab}]")
    });

    let doc = extract_doc_comment(&item_struct.attrs);

    let options: Vec<ConfigOptionItem> = named
        .named
        .iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let key = ident.to_string();
            let val_type = field.ty.to_token_stream().to_string();
            let opt_doc = extract_doc_comment(&field.attrs);
            Some(ConfigOptionItem::new(key, val_type, None, opt_doc))
        })
        .collect();

    Some(ConfigSectionItem::new(sec_name, file_fmt, doc, options))
}

fn parse_config_file_path(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if !attr.path().is_ident("config_file") {
            continue;
        }

        if let Ok(lit) = attr.parse_args::<Lit>()
            && let Lit::Str(s) = lit
        {
            return Some(s.value());
        }

        let mut path_str = None;
        drop(attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("path") || meta.path.is_ident("file") {
                let val: syn::LitStr = meta.value()?.parse()?;
                path_str = Some(val.value());
            }
            Ok(())
        }));

        if path_str.is_some() {
            return path_str;
        }
    }
    None
}

fn parse_config_section_name(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("config_section")
            && let Ok(lit) = attr.parse_args::<Lit>()
            && let Lit::Str(s) = lit
        {
            let val = s.value();
            return Some(if val.starts_with('[') {
                val
            } else {
                format!("[{val}]")
            });
        }

        if attr.path().is_ident("config_file") {
            let mut sec = None;
            drop(attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("section") {
                    let val: syn::LitStr = meta.value()?.parse()?;
                    sec = Some(val.value());
                }
                Ok(())
            }));
            if let Some(s) = sec {
                return Some(if s.starts_with('[') {
                    s
                } else {
                    format!("[{s}]")
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn extract_from_file_extracts_purist_lint_and_file_line_sections()
    -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            pub struct PuristLintsConfig {
                pub no_inline_mods: RuleLevel,
                pub max_file_lines: RuleLevel,
            }

            pub struct MaxFileLinesConfig {
                pub max_production_lines: Option<usize>,
                pub max_total_lines: Option<usize>,
            }
        "#;

        let file = syn::parse_file(code)?;
        let extractor = ConfigExtractor::new();
        let mut sections = Vec::new();
        extractor.extract_from_file(&file, &mut sections);

        expect_that!(sections.len(), eq(2));

        let lints_sec = sections.iter().find(|s| s.section == "[lints.purist]");
        assert_that!(lints_sec, some(anything()));
        let ls = lints_sec.ok_or("lints section missing")?;
        expect_that!(ls.file_format, eq("Cargo.toml"));
        expect_that!(ls.options.len(), eq(2));

        let lines_sec = sections
            .iter()
            .find(|s| s.section == "[package.metadata.purist]");
        assert_that!(lines_sec, some(anything()));
        let lls = lines_sec.ok_or("lines section missing")?;
        expect_that!(lls.options.len(), eq(2));

        Ok(())
    }

    #[googletest::test]
    fn extract_from_file_extracts_custom_config_file_derive()
    -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            /// Custom application settings
            #[derive(ConfigFile)]
            #[config_file("app.toml")]
            #[config_section("settings")]
            pub struct AppConfig {
                pub timeout_ms: u64,
                pub enabled: bool,
            }
        "#;

        let file = syn::parse_file(code)?;
        let extractor = ConfigExtractor::new();
        let mut sections = Vec::new();
        extractor.extract_from_file(&file, &mut sections);

        expect_that!(sections.len(), eq(1));
        let sec = sections.first().ok_or("custom section missing")?;
        expect_that!(sec.section, eq("[settings]"));
        expect_that!(sec.file_format, eq("app.toml"));
        expect_that!(sec.options.len(), eq(2));
        expect_that!(
            sec.doc,
            eq(&Some("Custom application settings".to_string()))
        );

        Ok(())
    }
}
