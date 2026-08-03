//! Supply-chain audit metadata for `ode_solvers`.

use gate_1b0_contract::audit_direct_dependency;
use gate_1b0_contract::DependencyAudit;

pub fn dependency_audit(tree: &str) -> DependencyAudit {
    audit_direct_dependency(
        "gate-1b0-ode-solvers",
        "ode_solvers",
        tree,
        "https://github.com/srenevey/ode-solvers",
        "v0.6.1",
        "Mature explicit RK crate; Dop853 present; ContinuousOutputModel \
         documented for Dopri5 only.",
        "Pulls nalgebra/simba/num-*; no native code.",
    )
    .expect("gate-1b0-ode-solvers dependency audit")
}
