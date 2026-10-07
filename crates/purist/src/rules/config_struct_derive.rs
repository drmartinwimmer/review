//! # Rule: `purist::config_struct_derive`
//!
//! ## What This Rule Does
//! Enforces that all configuration structs (structs containing "Config" or "config" in their name)
//! derive or implement the [`code_review_config::ConfigFile`] trait with an explicit
//! `#[config_file("...")]` attribute.
//!
//! ## Why This Rule Exists
//! Explicitly declaring the backing configuration file on configuration schema structures makes
//! configuration discovery uniform, machine-readable, and robust. It eliminates hard-coded
//! configuration file assumptions and allows tooling, API manifest generators, and validators
//! to reliably introspect all configuration options supported across the workspace.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! pub struct ServerConfig {
//!     pub host: String,
//!     pub port: u16,
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! use code_review_config::ConfigFile;
//!
//! #[derive(ConfigFile)]
//! #[config_file("config/server.toml")]
//! pub struct ServerConfig {
//!     pub host: String,
//!     pub port: u16,
//! }
//! ```

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use std::collections::HashSet;
use syn::visit::{self, Visit};

/// Rule requiring structs with "config" in their name to derive or implement `ConfigFile`.
pub struct ConfigStructDeriveRule;

impl Rule for ConfigStructDeriveRule {
    fn name(&self) -> &'static str {
        "purist::config_struct_derive"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut collector = ImplCollector {
            implemented_structs: HashSet::new(),
        };
        collector.visit_file(file);

        let mut visitor = ConfigStructVisitor {
            ctx,
            implemented_structs: collector.implemented_structs,
            in_test_scope: false,
            diagnostics: Vec::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that collects all structs that manually implement `ConfigFile`.
struct ImplCollector {
    implemented_structs: HashSet<String>,
}

impl<'ast> Visit<'ast> for ImplCollector {
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        if let Some((_, trait_path, _)) = &item_impl.trait_ {
            let is_config_file = trait_path
                .segments
                .last()
                .map(|s| s.ident == "ConfigFile")
                .unwrap_or(false);

            if is_config_file
                && let syn::Type::Path(type_path) = &*item_impl.self_ty
                && let Some(ident) = type_path.path.get_ident()
            {
                self.implemented_structs.insert(ident.to_string());
            }
        }
        visit::visit_item_impl(self, item_impl);
    }
}

/// Visitor that validates struct names and verifies `ConfigFile` derivation.
struct ConfigStructVisitor<'a> {
    ctx: &'a LintContext<'a>,
    implemented_structs: HashSet<String>,
    in_test_scope: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for ConfigStructVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_test = is_cfg_test_attr(&item_mod.attrs);
        let prev_test = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }
        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = prev_test;
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test = has_test_attr(&item_fn.attrs);
        let prev_test = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }
        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = prev_test;
    }

    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        if self.in_test_scope {
            return;
        }

        let name = item_struct.ident.to_string();
        if is_config_struct_name(&name) {
            let derives = derives_config_file(&item_struct.attrs);
            let manually_impls = self.implemented_structs.contains(&name);

            if !derives && !manually_impls {
                let span = self.ctx.to_span(item_struct.ident.span());
                self.diagnostics.push(
                    Diagnostic::new(
                        "purist::config_struct_derive",
                        Severity::Warning,
                        format!(
                            "Struct '{name}' has 'Config' in its name but does not implement or derive 'ConfigFile'."
                        ),
                    )
                    .with_span(span)
                    .with_suggested_fix(
                        "Add #[derive(code_review_config::ConfigFile)] and #[config_file(\"path/to/file\")] to declare its backing configuration file.",
                    ),
                );
            }
        }

        visit::visit_item_struct(self, item_struct);
    }
}

/// Determines whether a struct name indicates a configuration schema structure.
fn is_config_struct_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if !lower.contains("config") {
        return false;
    }

    // Exempt helper types, linters, commands, errors, and visitors
    if name.ends_with("Rule")
        || name.ends_with("Visitor")
        || name.ends_with("Error")
        || name.ends_with("Command")
        || name.ends_with("Args")
        || name.ends_with("Result")
    {
        return false;
    }

    true
}

/// Checks whether an attribute list derives `ConfigFile`.
fn derives_config_file(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let mut has_config_file = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("ConfigFile")
                || meta
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident == "ConfigFile")
                    .unwrap_or(false)
            {
                has_config_file = true;
            }
            Ok(())
        });
        has_config_file
    })
}

/// Checks whether an attribute list includes `#[cfg(test)]`.
fn is_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        let mut test_attr = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("test") {
                test_attr = true;
            }
            Ok(())
        });
        test_attr
    })
}

/// Checks whether an attribute list includes `#[test]` or `#[googletest::test]`.
fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || attr
                .path()
                .segments
                .last()
                .map(|s| s.ident == "test")
                .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn struct_deriving_config_file_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(Debug, ConfigFile)]
#[config_file("Cargo.toml")]
pub struct AppConfig {
    pub name: String,
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = ConfigStructDeriveRule;
        let diags = rule.check_file(&ctx, &file);
        expect_that!(&diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn struct_with_config_name_without_derive_is_flagged() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
pub struct DatabaseConfig {
    pub url: String,
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = ConfigStructDeriveRule;
        let diags = rule.check_file(&ctx, &file);
        let diag = diags.first().ok_or("expected diagnostic")?;
        expect_that!(diags.len(), eq(1));
        expect_that!(&diag.rule, eq("purist::config_struct_derive"));
        expect_that!(&diag.message, contains_substring("DatabaseConfig"));
        Ok(())
    }

    #[googletest::test]
    fn non_schema_structs_ending_with_rule_or_visitor_are_exempt()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct NoEnvAccessOutsideConfigRule;
pub struct ConfigVisitor;
pub struct ConfigError;
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = ConfigStructDeriveRule;
        let diags = rule.check_file(&ctx, &file);
        expect_that!(&diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn config_struct_in_test_module_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    struct TestConfig {
        foo: u32,
    }
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = ConfigStructDeriveRule;
        let diags = rule.check_file(&ctx, &file);
        expect_that!(&diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn manual_impl_config_file_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct ManualConfig {
    pub port: u16,
}

impl ConfigFile for ManualConfig {
    fn config_file_path() -> &'static str {
        "manual.json"
    }
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = ConfigStructDeriveRule;
        let diags = rule.check_file(&ctx, &file);
        expect_that!(&diags, is_empty());
        Ok(())
    }
}
