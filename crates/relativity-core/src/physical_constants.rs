//! Pinned physical constants for Gate 2C0 radiometry.
//!
//! Authority: CODATA 2018 / SI exact 2019 values where applicable, plus IAU 2015
//! Resolution B3 nominal solar GM. Revision string is hashed into digests.
//! Not geometrized `G=c=M=1`.

/// Digest-facing constants revision id (do not rename casually).
pub const CONSTANTS_REVISION: &str = "codata-2018+iau-b3-2015-v1";

/// Speed of light [m/s] (exact, SI).
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// Planck constant [J·s] (exact, SI 2019).
pub const PLANCK_H_J_S: f64 = 6.626_070_15e-34;

/// Boltzmann constant [J/K] (exact, SI 2019).
pub const BOLTZMANN_K_J_K: f64 = 1.380_649e-23;

/// Newtonian gravitational constant [m³ kg⁻¹ s⁻²] (CODATA 2018).
pub const GRAVITATIONAL_G_M3_KG_S2: f64 = 6.674_30e-11;

/// IAU 2015 Resolution B3 nominal solar GM [m³ s⁻²].
pub const GM_SUN_NOMINAL_M3_S2: f64 = 1.327_124_4e20;

/// Stefan–Boltzmann constant derived from exact `h`, `c`, `k_B`:
/// `σ = 2 π⁵ k⁴ / (15 c² h³)`.
#[must_use]
pub fn stefan_boltzmann_w_m2_k4() -> f64 {
    let pi = std::f64::consts::PI;
    let k = BOLTZMANN_K_J_K;
    let c = SPEED_OF_LIGHT_M_S;
    let h = PLANCK_H_J_S;
    let k2 = k * k;
    let k4 = k2 * k2;
    let h2 = h * h;
    let h3 = h2 * h;
    let c2 = c * c;
    let pi2 = pi * pi;
    let pi5 = pi2 * pi2 * pi;
    2.0 * pi5 * k4 / (15.0 * c2 * h3)
}

/// Nominal solar mass [kg] = `GM_☉ⁿ / G` with pinned CODATA `G`.
#[must_use]
pub fn solar_mass_kg() -> f64 {
    GM_SUN_NOMINAL_M3_S2 / GRAVITATIONAL_G_M3_KG_S2
}

/// Convert solar-mass multiples to kilograms via pinned IAU/CODATA path.
#[must_use]
pub fn mass_kg_from_solar_masses(solar_masses: f64) -> f64 {
    solar_masses * solar_mass_kg()
}

/// Geometrized length unit `GM/c²` [m] for a mass in kilograms.
#[must_use]
pub fn gravitational_radius_m(mass_kg: f64) -> f64 {
    GRAVITATIONAL_G_M3_KG_S2 * mass_kg / (SPEED_OF_LIGHT_M_S * SPEED_OF_LIGHT_M_S)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stefan_boltzmann_near_codata_2018() {
        let sigma = stefan_boltzmann_w_m2_k4();
        // CODATA 2018 recommended value 5.670374419e-8; derived from exact h,c,k.
        assert!((sigma - 5.670_374_419e-8).abs() / 5.670_374_419e-8 < 1e-9);
    }

    #[test]
    fn solar_mass_finite_positive() {
        let m = solar_mass_kg();
        assert!(m.is_finite() && m > 1.0e30 && m < 2.0e30);
    }
}
