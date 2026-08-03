//! Supply-chain audit metadata for `ivp`.

use gate_1b0_contract::DependencyAudit;
use sha2::{Digest, Sha256};

pub fn dependency_audit(tree: &str) -> DependencyAudit {
    DependencyAudit {
        crate_name: "ivp".into(),
        exact_version: super::DEP_VERSION.into(),
        license: "Apache-2.0".into(),
        source_repo: "https://github.com/Ryan-D-Gast/ivp".into(),
        source_tag_or_rev: "v0.6.0".into(),
        direct_unsafe_in_crate: false,
        build_scripts: vec!["build.rs (bon macros)".into()],
        native_dependencies: vec![],
        cargo_tree_digest: hex::encode(Sha256::digest(tree.as_bytes())),
        maintenance_notes: "Younger crate; DOP853 + SolOut + vector tol + dense output.".into(),
        transitive_risk_notes: "bon/darling proc-macros; matrix crate; no native code.".into(),
    }
}
