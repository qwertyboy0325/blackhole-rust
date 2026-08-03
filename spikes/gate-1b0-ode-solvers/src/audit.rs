//! Supply-chain audit metadata for `ode_solvers`.

use gate_1b0_contract::DependencyAudit;
use sha2::{Digest, Sha256};

pub fn dependency_audit(tree: &str) -> DependencyAudit {
    DependencyAudit {
        crate_name: "ode_solvers".into(),
        exact_version: super::DEP_VERSION.into(),
        license: "Apache-2.0".into(),
        source_repo: "https://github.com/srenevey/ode-solvers".into(),
        source_tag_or_rev: "v0.6.1".into(),
        direct_unsafe_in_crate: false,
        build_scripts: vec![],
        native_dependencies: vec![],
        cargo_tree_digest: hex::encode(Sha256::digest(tree.as_bytes())),
        maintenance_notes: "Mature explicit RK crate; Dop853 present; ContinuousOutputModel \
                            documented for Dopri5 only."
            .into(),
        transitive_risk_notes: "Pulls nalgebra/simba/num-*; no native code.".into(),
    }
}
