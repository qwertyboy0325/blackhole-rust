//! Reproducible supply-chain audit via `cargo metadata` and source scan.

use crate::schema::{DependencyAudit, UnsafeOccurrence};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn audit_direct_dependency(
    spike_package: &str,
    dep_crate: &str,
    tree: &str,
    repo: &str,
    tag: &str,
    maintenance: &str,
    transitive_notes: &str,
) -> Result<DependencyAudit, String> {
    let meta_json = cargo_metadata(spike_package)?;
    let pkg = find_dependency_package(&meta_json, dep_crate)?;
    let source_dir = package_source_dir(&pkg)?;
    let unsafe_occurrences = scan_unsafe(&source_dir)?;
    let build_scripts = find_build_scripts(&source_dir);
    let proc_macros = find_proc_macros(&meta_json, dep_crate);
    let native = find_native_links(&meta_json, dep_crate);

    Ok(DependencyAudit {
        crate_name: dep_crate.into(),
        exact_version: pkg.version.clone(),
        package_id: pkg.id.clone(),
        checksum: pkg.checksum.clone().unwrap_or_default(),
        source: pkg.source.clone().unwrap_or_else(|| "path".into()),
        license: pkg.license.clone().unwrap_or_else(|| "unknown".into()),
        source_repo: repo.into(),
        source_tag_or_rev: tag.into(),
        direct_unsafe_occurrences: unsafe_occurrences,
        build_scripts,
        proc_macro_crates: proc_macros,
        native_dependencies: native,
        cargo_tree_digest: hex::encode(Sha256::digest(tree.as_bytes())),
        audit_commands: vec![
            format!(
                "cargo metadata --manifest-path spikes/{spike_package}/Cargo.toml --format-version 1"
            ),
            format!("rg 'unsafe\\s+(fn|impl|{{|trait)' {}/src", source_dir.display()),
            format!("cargo tree -p {spike_package} --format '{{p}} {{v}}'"),
        ],
        maintenance_notes: maintenance.into(),
        transitive_risk_notes: transitive_notes.into(),
    })
}

#[derive(serde::Deserialize)]
struct Metadata {
    packages: Vec<MetaPackage>,
}

#[derive(serde::Deserialize)]
struct MetaPackage {
    id: String,
    name: String,
    version: String,
    license: Option<String>,
    source: Option<String>,
    checksum: Option<String>,
    manifest_path: String,
}

fn spike_manifest_path(spike_package: &str) -> Result<PathBuf, String> {
    let root = workspace_root()?;
    let path = root.join("spikes").join(spike_package).join("Cargo.toml");
    if path.exists() {
        Ok(path)
    } else {
        Err(format!(
            "manifest not found for spike package {spike_package}"
        ))
    }
}

fn cargo_metadata(spike_package: &str) -> Result<Metadata, String> {
    let root = workspace_root()?;
    let manifest = spike_manifest_path(spike_package)?;
    let out = Command::new("cargo")
        .current_dir(&root)
        .args([
            "metadata",
            "--manifest-path",
            manifest.to_str().ok_or("non-utf8 manifest path")?,
            "--format-version",
            "1",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

fn find_dependency_package(meta: &Metadata, dep_crate: &str) -> Result<MetaPackage, String> {
    meta.packages
        .iter()
        .find(|p| p.name == dep_crate)
        .cloned()
        .ok_or_else(|| format!("dependency {dep_crate} not in metadata"))
}

impl Clone for MetaPackage {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            license: self.license.clone(),
            source: self.source.clone(),
            checksum: self.checksum.clone(),
            manifest_path: self.manifest_path.clone(),
        }
    }
}

fn package_source_dir(pkg: &MetaPackage) -> Result<PathBuf, String> {
    let manifest = Path::new(&pkg.manifest_path);
    manifest
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "no package parent".into())
}

fn scan_unsafe(source_dir: &Path) -> Result<Vec<UnsafeOccurrence>, String> {
    let src = source_dir.join("src");
    if !src.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    walk_rs_files(&src, &mut |path, line_no, line| {
        let trimmed = line.trim();
        if trimmed.contains("unsafe fn")
            || trimmed.contains("unsafe impl")
            || trimmed.contains("unsafe trait")
            || trimmed.starts_with("unsafe {")
            || trimmed.starts_with("unsafe{")
        {
            out.push(UnsafeOccurrence {
                file: path
                    .strip_prefix(source_dir)
                    .unwrap_or(path)
                    .display()
                    .to_string(),
                line: line_no,
                kind: trimmed.chars().take(80).collect(),
            });
        }
    });
    Ok(out)
}

fn walk_rs_files(dir: &Path, f: &mut dyn FnMut(&Path, u32, &str)) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_rs_files(&path, f);
            } else if path.extension().is_some_and(|e| e == "rs") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    for (i, line) in text.lines().enumerate() {
                        f(&path, (i + 1) as u32, line);
                    }
                }
            }
        }
    }
}

fn find_build_scripts(source_dir: &Path) -> Vec<String> {
    let build_rs = source_dir.join("build.rs");
    if build_rs.exists() {
        vec![build_rs.display().to_string()]
    } else {
        vec![]
    }
}

fn find_proc_macros(meta: &Metadata, dep_crate: &str) -> Vec<String> {
    meta.packages
        .iter()
        .filter(|p| {
            p.name != dep_crate
                && std::fs::read_to_string(&p.manifest_path)
                    .map(|m| m.contains("[lib]") && m.contains("proc-macro = true"))
                    .unwrap_or(false)
        })
        .map(|p| format!("{}@{}", p.name, p.version))
        .collect()
}

fn find_native_links(meta: &Metadata, dep_crate: &str) -> Vec<String> {
    meta.packages
        .iter()
        .filter(|p| p.name == dep_crate)
        .filter_map(|p| {
            std::fs::read_to_string(&p.manifest_path)
                .ok()
                .and_then(|m| {
                    m.lines()
                        .find(|l| l.starts_with("links"))
                        .map(|l| l.to_string())
                })
        })
        .collect()
}

fn workspace_root() -> Result<PathBuf, String> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    Ok(dir)
}
