use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImplAnnotation {
    /// Requirement ID (qualified name or short name as written by developer)
    pub req_id: String,
    /// Name of the annotated item (fn name, struct name, etc.)
    pub item_name: String,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifyAnnotation {
    pub req_id: String,
    pub test_name: String,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct CollectedAnnotations {
    pub implementations: Vec<ImplAnnotation>,
    pub verifications: Vec<VerifyAnnotation>,
}

pub fn collect_annotations(src_dir: &Path) -> Result<CollectedAnnotations> {
    let mut collected = CollectedAnnotations::default();

    for entry in walkdir::WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
    {
        if let Err(e) = collect_from_file(entry.path(), &mut collected) {
            eprintln!("warning: skipping {:?}: {}", entry.path(), e);
        }
    }

    Ok(collected)
}

fn collect_from_file(path: &Path, collected: &mut CollectedAnnotations) -> Result<()> {
    let source = std::fs::read_to_string(path)?;
    let file: syn::File = syn::parse_str(&source)?;

    let mut visitor = AnnotationVisitor { path, collected };
    syn::visit::visit_file(&mut visitor, &file);

    Ok(())
}

struct AnnotationVisitor<'a> {
    path: &'a Path,
    collected: &'a mut CollectedAnnotations,
}

impl<'a> AnnotationVisitor<'a> {
    fn process_attrs(&mut self, attrs: &[syn::Attribute], name: &str, line: usize) {
        for attr in attrs {
            if let Some(ids) = extract_sysgen_attr(attr, "implements") {
                for id in ids {
                    self.collected.implementations.push(ImplAnnotation {
                        req_id: id,
                        item_name: name.to_string(),
                        file: self.path.to_path_buf(),
                        line,
                    });
                }
            }
            if let Some(ids) = extract_sysgen_attr(attr, "verifies") {
                for id in ids {
                    self.collected.verifications.push(VerifyAnnotation {
                        req_id: id,
                        test_name: name.to_string(),
                        file: self.path.to_path_buf(),
                        line,
                    });
                }
            }
        }
    }
}

impl<'a, 'ast> syn::visit::Visit<'ast> for AnnotationVisitor<'a> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let name = node.sig.ident.to_string();
        let line = get_line(&node.sig.ident);
        self.process_attrs(&node.attrs, &name, line);
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let name = node.ident.to_string();
        let line = get_line(&node.ident);
        self.process_attrs(&node.attrs, &name, line);
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let name = match &*node.self_ty {
            syn::Type::Path(tp) => tp
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        let line = node.impl_token.span.start().line;
        self.process_attrs(&node.attrs, &name, line);
        syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let name = node.ident.to_string();
        let line = get_line(&node.ident);
        self.process_attrs(&node.attrs, &name, line);
        syn::visit::visit_item_mod(self, node);
    }
}

/// Extract req IDs from `#[implements("R1", "R2")]` or `#[doc = "sysgen:implements:R1"]`.
fn extract_sysgen_attr(attr: &syn::Attribute, kind: &str) -> Option<Vec<String>> {
    // Check path-style: #[implements("...")] or #[verifies("...")]
    if attr.path().is_ident(kind) {
        if let Ok(ids) = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated,
        ) {
            return Some(ids.into_iter().map(|s| s.value()).collect());
        }
    }

    // Check doc-style: #[doc = "sysgen:implements:VehicleRequirements::MassRequirement"]
    if attr.path().is_ident("doc") {
        if let syn::Meta::NameValue(nv) = &attr.meta {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
            {
                let val = s.value();
                let prefix = format!("sysgen:{}:", kind);
                if val.starts_with(&prefix) {
                    return Some(vec![val[prefix.len()..].to_string()]);
                }
            }
        }
    }

    None
}

fn get_line(ident: &syn::Ident) -> usize {
    ident.span().start().line
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_fixture(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn collects_implements_from_fn() {
        let dir = TempDir::new().unwrap();
        write_fixture(
            &dir,
            "lib.rs",
            r#"
            use sysgen_macros::implements;

            #[implements("VehicleRequirements::MassRequirement")]
            pub fn check_mass() -> bool { true }
        "#,
        );

        let annotations = collect_annotations(dir.path()).unwrap();
        assert_eq!(annotations.implementations.len(), 1);
        assert_eq!(
            annotations.implementations[0].req_id,
            "VehicleRequirements::MassRequirement"
        );
        assert_eq!(annotations.implementations[0].item_name, "check_mass");
    }

    #[test]
    fn collects_verifies_from_test_fn() {
        let dir = TempDir::new().unwrap();
        write_fixture(
            &dir,
            "tests.rs",
            r#"
            #[verifies("VehicleRequirements::MassRequirement")]
            #[test]
            fn test_mass() { assert!(true); }
        "#,
        );

        let annotations = collect_annotations(dir.path()).unwrap();
        assert_eq!(annotations.verifications.len(), 1);
        assert_eq!(
            annotations.verifications[0].req_id,
            "VehicleRequirements::MassRequirement"
        );
    }

    #[test]
    fn collects_multi_req_ids() {
        let dir = TempDir::new().unwrap();
        write_fixture(
            &dir,
            "lib.rs",
            r#"
            #[implements("R1", "R2")]
            pub fn check_both() {}
        "#,
        );

        let annotations = collect_annotations(dir.path()).unwrap();
        assert_eq!(annotations.implementations.len(), 2);
    }

    #[test]
    fn collects_struct_annotations() {
        let dir = TempDir::new().unwrap();
        write_fixture(
            &dir,
            "lib.rs",
            r#"
            #[implements("StructReq")]
            pub struct MyStruct {}
        "#,
        );

        let annotations = collect_annotations(dir.path()).unwrap();
        assert_eq!(annotations.implementations.len(), 1);
        assert_eq!(annotations.implementations[0].item_name, "MyStruct");
    }

    #[test]
    fn collects_doc_style_annotations() {
        let dir = TempDir::new().unwrap();
        write_fixture(
            &dir,
            "lib.rs",
            r#"
            #[doc = "sysgen:implements:VehicleRequirements::MassRequirement"]
            pub fn check_mass() -> bool { true }
        "#,
        );

        let annotations = collect_annotations(dir.path()).unwrap();
        assert_eq!(annotations.implementations.len(), 1);
        assert_eq!(
            annotations.implementations[0].req_id,
            "VehicleRequirements::MassRequirement"
        );
    }

    #[test]
    fn skips_unparseable_files_without_aborting() {
        let dir = TempDir::new().unwrap();
        write_fixture(&dir, "bad.rs", "this is not valid rust !!@#$");
        write_fixture(
            &dir,
            "good.rs",
            r#"
            #[implements("R1")]
            pub fn ok() {}
        "#,
        );

        let annotations = collect_annotations(dir.path()).unwrap();
        assert_eq!(annotations.implementations.len(), 1);
    }
}
