//! Independent BL ↔ Cartesian KS validation (not mere inverse of production helpers).

use relativity_core::{
    bl_metric, bl_to_ks_position, covector_bl_to_ks, covector_ks_to_bl, evaluate_kerr_schild,
    jacobian_cartesian_ks_from_bl, vector_bl_to_ks, vector_ks_to_bl, zamo_observer, Covector,
    KerrParams, PositionBl, Vector,
};

fn pullback_residual(j: &[[f64; 4]; 4], g_ks: &[[f64; 4]; 4], g_bl: &[[f64; 4]; 4]) -> f64 {
    // (Jᵀ g_KS J)_{αβ} − (g_BL)_{αβ}
    let mut max: f64 = 0.0;
    for alpha in 0..4 {
        for beta in 0..4 {
            let mut s = 0.0;
            for mu in 0..4 {
                for nu in 0..4 {
                    s += j[mu][alpha] * g_ks[mu][nu] * j[nu][beta];
                }
            }
            max = max.max((s - g_bl[alpha][beta]).abs());
        }
    }
    max
}

#[test]
fn metric_pullback_matches_independent_bl_metric() {
    let cases = [
        (1.0, 0.5, 12.0, 1.0, 0.3),
        (1.0, 0.9, 8.0, 1.2, -0.4),
        (1.0, 0.999, 20.0, 85.0_f64.to_radians(), 0.0),
        (1.0, 0.0, 15.0, 0.8, 1.1),
    ];
    let mut worst = 0.0_f64;
    for &(m, a, r, theta, phi) in &cases {
        let params = KerrParams::new(m, a).unwrap();
        let bl = PositionBl::new(0.0, r, theta, phi);
        let g_bl = bl_metric(&params, &bl).unwrap();
        let ks = bl_to_ks_position(&params, &bl).unwrap();
        let g_ks = evaluate_kerr_schild(&params, &ks).unwrap().metric;
        let j = jacobian_cartesian_ks_from_bl(&params, &bl).unwrap();
        let res = pullback_residual(&j, &g_ks.components(), &g_bl.components());
        worst = worst.max(res);
        assert!(
            res < 1e-8,
            "pullback residual {res} at r={r} a={a} θ={theta}"
        );

        // Explicit radial time/azimuth exterior terms.
        let delta = r * r - 2.0 * m * r + a * a;
        assert!((j[0][1] - 2.0 * m * r / delta).abs() < 1e-12);
    }
    eprintln!("worst Jᵀ g_KS J − g_BL residual = {worst:.3e}");
}

#[test]
fn vector_covector_pairing_and_round_trips() {
    let params = KerrParams::new(1.0, 0.8).unwrap();
    let bl = PositionBl::new(0.0, 10.0, 1.1, 0.5);
    let v = Vector::new(1.2, -0.3, 0.4, 0.1);
    let p = Covector::new(-0.9, 0.2, -0.1, 0.3);

    let v_ks = vector_bl_to_ks(&params, &bl, &v).unwrap();
    let v_back = vector_ks_to_bl(&params, &bl, &v_ks).unwrap();
    for (a, b) in v.components().iter().zip(v_back.components()) {
        assert!((a - b).abs() < 1e-10, "vector RT");
    }

    let p_ks = covector_bl_to_ks(&params, &bl, &p).unwrap();
    let p_back = covector_ks_to_bl(&params, &bl, &p_ks).unwrap();
    for (a, b) in p.components().iter().zip(p_back.components()) {
        assert!((a - b).abs() < 1e-10, "covector RT");
    }

    // Pairing invariance: p_μ v^μ equal in both charts.
    let pair_bl: f64 = p
        .components()
        .iter()
        .zip(v.components())
        .map(|(a, b)| a * b)
        .sum();
    let pair_ks: f64 = p_ks
        .components()
        .iter()
        .zip(v_ks.components())
        .map(|(a, b)| a * b)
        .sum();
    assert!(
        (pair_bl - pair_ks).abs() < 1e-10,
        "pairing {pair_bl} vs {pair_ks}"
    );
}

#[test]
fn zamo_zero_angular_momentum_and_pullback_norm() {
    let params = KerrParams::new(1.0, 0.999).unwrap();
    let bl = PositionBl::new(0.0, 20.0, 85.0_f64.to_radians(), 0.0);
    let obs = zamo_observer(&params, &bl).unwrap();
    let u_phi = obs.bl_u_phi.unwrap();
    assert!(u_phi.abs() < 1e-12, "u_φ={u_phi}");

    let g_bl = bl_metric(&params, &bl).unwrap();
    // Reconstruct BL four-velocity from ZAMO formula and check g(u,u).
    let m = params.mass();
    let a = params.spin();
    let r = bl.r;
    let sth = bl.theta.sin();
    let cth = bl.theta.cos();
    let sigma = r * r + a * a * cth * cth;
    let delta = r * r - 2.0 * m * r + a * a;
    let a_factor = (r * r + a * a).powi(2) - delta * a * a * sth * sth;
    let omega = 2.0 * m * a * r / a_factor;
    let u_t = (a_factor / (delta * sigma)).sqrt();
    let u_bl = Vector::new(u_t, 0.0, 0.0, omega * u_t);
    assert!((g_bl.contract(&u_bl, &u_bl) + 1.0).abs() < 1e-12);

    let u_ks = vector_bl_to_ks(&params, &bl, &u_bl).unwrap();
    let g_ks = evaluate_kerr_schild(&params, &obs.event).unwrap().metric;
    assert!((g_ks.contract(&u_ks, &u_ks) + 1.0).abs() < 1e-10);
}
