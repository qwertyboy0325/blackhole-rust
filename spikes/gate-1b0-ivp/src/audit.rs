//! Supply-chain audit metadata for `ivp`.

use gate_1b0_contract::audit_direct_dependency;
use gate_1b0_contract::DependencyAudit;

pub fn dependency_audit(tree: &str) -> DependencyAudit {
    audit_direct_dependency(
        "gate-1b0-ivp",
        "ivp",
        tree,
        "https://github.com/Ryan-D-Gast/ivp",
        "v0.6.0",
        "Younger crate; DOP853 + SolOut + vector tol + dense output.",
        "bon/darling proc-macros; matrix crate; no native code.",
    )
    .expect("gate-1b0-ivp dependency audit")
}
