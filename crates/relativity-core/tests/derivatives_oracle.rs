//! Independent central finite-difference oracle for `∂_i g^{αβ}`.
//!
//! This file must not call `inverse_metric_spatial_derivatives` for the oracle
//! values — only for the production analytic result under test.
//!
//! Tolerance provenance: FD truncation + fp noise from a step-size sweep; not a
//! calibrated scientific acceptance threshold for production integration.

use relativity_core::{
    evaluate_kerr_schild, inverse_metric_spatial_derivatives, stratified_corpus, CorpusTag,
    ExpectedOutcome, KerrParams, PositionKs, CORPUS_SEED,
};

/// Central-difference step scaled to the local coordinate magnitude.
///
/// Fixed `h = 1e-6` is invalid in the cancellation-prone oblate regime where
/// `|z| ≪ 1e-6`: the stencil leaves the locally linear neighborhood and the
/// FD value is not an estimate of `∂_z` at the sample point (observed factor
/// ~`h/|z|` disagreement vs analytic). Provenance: conditioning experiment in
/// `docs/work-log/gate-1a-layer-notes.md`.
fn fd_step(coord: f64, r: f64) -> f64 {
    // Scale with both the differentiation coordinate and the oblate radius so
    // cancellation-prone points (|z|≪1, r∼|z|) keep the stencil local.
    let scale = coord.abs().max(r).max(1e-16);
    let h = 1e-6 * scale;
    h.clamp(1e-14, 1e-4)
}

fn fd_partial_ginv(params: &KerrParams, pos: &PositionKs, axis: usize) -> Option<[[f64; 4]; 4]> {
    let r = evaluate_kerr_schild(params, pos).ok()?.radius.r;
    let coord = match axis {
        0 => pos.x,
        1 => pos.y,
        2 => pos.z,
        _ => unreachable!(),
    };
    let h = fd_step(coord, r);
    let mut plus = *pos;
    let mut minus = *pos;
    match axis {
        0 => {
            plus.x += h;
            minus.x -= h;
        }
        1 => {
            plus.y += h;
            minus.y -= h;
        }
        2 => {
            plus.z += h;
            minus.z -= h;
        }
        _ => unreachable!(),
    }
    let gp = evaluate_kerr_schild(params, &plus).ok()?.inverse_metric;
    let gm = evaluate_kerr_schild(params, &minus).ok()?.inverse_metric;
    let mut out = [[0.0; 4]; 4];
    for a in 0..4 {
        for b in 0..4 {
            out[a][b] = (gp.get(a, b) - gm.get(a, b)) / (2.0 * h);
        }
    }
    Some(out)
}

#[derive(Debug)]
struct Worst {
    abs: f64,
    rel: f64,
    tag: CorpusTag,
    axis: usize,
    alpha: usize,
    beta: usize,
    pos: PositionKs,
    spin: f64,
}

#[test]
fn analytic_derivatives_match_fd_oracle() {
    // Tolerances from FD/oracle comparison with adaptive step on the stratified
    // corpus. Worst residuals occur in CancellationProneOblate; these are
    // oracle-comparison bounds, not owner-approved geodesic acceptance thresholds.
    let abs_tol = 5e-3;
    let rel_tol = 2e-3;

    let mut worst_abs = Worst {
        abs: 0.0,
        rel: 0.0,
        tag: CorpusTag::WeakField,
        axis: 0,
        alpha: 0,
        beta: 0,
        pos: PositionKs::spatial(0.0, 0.0, 0.0),
        spin: 0.0,
    };
    let mut worst_rel = 0.0_f64;
    let mut worst_rel_rec = Worst {
        abs: 0.0,
        rel: 0.0,
        tag: CorpusTag::WeakField,
        axis: 0,
        alpha: 0,
        beta: 0,
        pos: PositionKs::spatial(0.0, 0.0, 0.0),
        spin: 0.0,
    };

    let mut compared = 0u64;
    for pt in stratified_corpus() {
        if !matches!(pt.expected, ExpectedOutcome::Valid) {
            continue;
        }
        let params = pt.params().unwrap();
        let Ok(analytic) = inverse_metric_spatial_derivatives(&params, &pt.pos) else {
            panic!("valid corpus point failed analytic ∂ at {:?}", pt.pos);
        };
        for axis in 0..3 {
            let Some(fd) = fd_partial_ginv(&params, &pt.pos, axis) else {
                continue;
            };
            for a in 0..4 {
                for b in 0..4 {
                    let an = analytic.spatial[axis][a][b];
                    let diff = (an - fd[a][b]).abs();
                    let scale = an.abs().max(fd[a][b].abs()).max(1e-12);
                    let rel = diff / scale;
                    compared += 1;
                    if diff > worst_abs.abs {
                        worst_abs = Worst {
                            abs: diff,
                            rel,
                            tag: pt.tag,
                            axis,
                            alpha: a,
                            beta: b,
                            pos: pt.pos,
                            spin: params.spin(),
                        };
                    }
                    if rel > worst_rel {
                        worst_rel = rel;
                        worst_rel_rec = Worst {
                            abs: diff,
                            rel,
                            tag: pt.tag,
                            axis,
                            alpha: a,
                            beta: b,
                            pos: pt.pos,
                            spin: params.spin(),
                        };
                    }
                    assert!(
                        diff <= abs_tol || rel <= rel_tol,
                        "derivative mismatch abs={diff} rel={rel} tag={:?} axis={axis} αβ=({a},{b}) \
                         pos=({},{};{}) a={} seed={CORPUS_SEED} analytic={an} fd={}",
                        pt.tag,
                        pt.pos.x,
                        pt.pos.y,
                        pt.pos.z,
                        params.spin(),
                        fd[a][b]
                    );
                }
            }
        }
    }

    assert!(compared > 100, "expected substantial corpus comparisons");
    eprintln!(
        "derivative oracle worst abs={} rel={} at {:?} axis={} αβ=({},{}) pos=({:.6},{:.6},{:.6}) a={} seed={}",
        worst_abs.abs,
        worst_abs.rel,
        worst_abs.tag,
        worst_abs.axis,
        worst_abs.alpha,
        worst_abs.beta,
        worst_abs.pos.x,
        worst_abs.pos.y,
        worst_abs.pos.z,
        worst_abs.spin,
        CORPUS_SEED
    );
    eprintln!(
        "derivative oracle worst rel={} abs={} at {:?} axis={} αβ=({},{}) pos=({:.6},{:.6},{:.6}) a={}",
        worst_rel_rec.rel,
        worst_rel_rec.abs,
        worst_rel_rec.tag,
        worst_rel_rec.axis,
        worst_rel_rec.alpha,
        worst_rel_rec.beta,
        worst_rel_rec.pos.x,
        worst_rel_rec.pos.y,
        worst_rel_rec.pos.z,
        worst_rel_rec.spin
    );
}

#[test]
fn fd_step_sweep_documents_noise_floor() {
    let params = KerrParams::new(1.0, 0.7).unwrap();
    let pos = PositionKs::spatial(6.0, 1.0, 2.0);
    let analytic = inverse_metric_spatial_derivatives(&params, &pos).unwrap();
    let fd = fd_partial_ginv(&params, &pos, 0).unwrap();
    let mut max: f64 = 0.0;
    for a in 0..4 {
        for b in 0..4 {
            max = max.max((analytic.spatial[0][a][b] - fd[a][b]).abs());
        }
    }
    eprintln!("adaptive FD max_abs_err={max} (mid-field ∂x)");
    assert!(max < 1e-5, "adaptive FD disagree; max_abs_err={max}");
}

#[test]
fn fixed_h_invalid_in_cancellation_regime() {
    // Evidence: fixed h=1e-6 at z=1e-8 is not a valid ∂_z probe.
    let params = KerrParams::new(1.0, 0.999).unwrap();
    let pos = PositionKs::spatial(0.1, 0.0, 1e-8);
    let analytic = inverse_metric_spatial_derivatives(&params, &pos).unwrap();
    let mut plus = pos;
    let mut minus = pos;
    let h_bad = 1e-6;
    plus.z += h_bad;
    minus.z -= h_bad;
    let gp = evaluate_kerr_schild(&params, &plus).unwrap().inverse_metric;
    let gm = evaluate_kerr_schild(&params, &minus)
        .unwrap()
        .inverse_metric;
    let fd_bad = (gp.get(0, 0) - gm.get(0, 0)) / (2.0 * h_bad);
    let an = analytic.spatial[2][0][0];
    let rel_bad = (an - fd_bad).abs() / an.abs().max(fd_bad.abs()).max(1e-30);
    assert!(
        rel_bad > 0.1,
        "expected fixed-h stencil to be invalid; rel={rel_bad}"
    );
    let fd_ok = fd_partial_ginv(&params, &pos, 2).unwrap();
    let rel_ok = (an - fd_ok[0][0]).abs() / an.abs().max(fd_ok[0][0].abs()).max(1e-30);
    assert!(rel_ok < 2e-3, "adaptive FD should agree; rel={rel_ok}");
}
