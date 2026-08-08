//! Gate 2D1 procedural celestial environment (ARTISTIC_ENVIRONMENT).
//!
//! Samples finite-boundary `unit_coordinate_direction` — not null infinity.
//! Label: `MILKY_WAY_LIKE_PROCEDURAL_APPEARANCE`. No external image assets.
//! Star profiles use fixed angular sigma (A1) — never pixel-relative.

use crate::error::AppearanceError;
use crate::tone_map::LinearRgb;
use relativity_core::SphericalKsAzimuthStatus;
use relativity_trace::{hex_sha, CelestialBoundarySample, CelestialDirectionSource, CelestialUv};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ENVIRONMENT_MODEL_ID: &str = "procedural-hdr-direction-domain-v1";
pub const MILKY_WAY_LABEL: &str = "MILKY_WAY_LIKE_PROCEDURAL_APPEARANCE";
pub const STAR_PROFILE_ID: &str = "angular-gaussian-v1";
pub const HASH_PRNG_ID: &str = "splitmix64-v1";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UnitQuaternion {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl UnitQuaternion {
    pub fn identity() -> Self {
        Self {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn validate(&self) -> Result<(), AppearanceError> {
        let n2 = self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z;
        if !n2.is_finite() || (n2 - 1.0).abs() > 1e-9 {
            return Err(AppearanceError::InvalidSpec(format!(
                "environment_rotation must be unit quaternion; |q|²={n2}"
            )));
        }
        Ok(())
    }

    /// Rotate vector by unit quaternion: `v' = q v q⁻¹`.
    pub fn rotate_vector(self, v: [f64; 3]) -> [f64; 3] {
        let u = [self.x, self.y, self.z];
        let w = self.w;
        let dot_uv = u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
        let cross = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let uu = u[0] * u[0] + u[1] * u[1] + u[2] * u[2];
        [
            2.0 * dot_uv * u[0] + (w * w - uu) * v[0] + 2.0 * w * cross[0],
            2.0 * dot_uv * u[1] + (w * w - uu) * v[1] + 2.0 * w * cross[1],
            2.0 * dot_uv * u[2] + (w * w - uu) * v[2] + 2.0 * w * cross[2],
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilkyWayLikeSpec {
    pub label: String,
    /// Unit pole of the great-circle band plane (band is equator of this pole).
    pub pole: [f64; 3],
    pub band_sigma_rad: f64,
    pub band_peak: f64,
    pub core_sigma_rad: f64,
    pub core_peak: f64,
    pub dust_sigma_rad: f64,
    pub dust_depth: f64,
    pub longitude_modulation_amp: f64,
    pub longitude_harmonics: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StarsSpec {
    pub profile_id: String,
    pub seed: u64,
    pub algorithm_id: String,
    pub count: u32,
    /// Fixed angular Gaussian sigma (A1) — independent of render resolution.
    pub angular_sigma_rad: f64,
    pub peak_scale: f64,
    pub band_bias: f64,
    pub t_min_k: f64,
    pub t_max_k: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    pub model_id: String,
    pub environment_rotation: UnitQuaternion,
    pub sky_floor: f64,
    pub milky_way: MilkyWayLikeSpec,
    pub stars: StarsSpec,
    /// When true, environment samples return black (identity / 2D0 differential).
    pub identity_black: bool,
}

impl EnvironmentSpec {
    pub fn validate(&self) -> Result<(), AppearanceError> {
        if self.model_id != ENVIRONMENT_MODEL_ID {
            return Err(AppearanceError::InvalidSpec(format!(
                "unsupported environment model {}",
                self.model_id
            )));
        }
        self.environment_rotation.validate()?;
        if !self.sky_floor.is_finite() || self.sky_floor < 0.0 {
            return Err(AppearanceError::InvalidSpec(
                "sky_floor must be finite and >= 0".into(),
            ));
        }
        if self.milky_way.label != MILKY_WAY_LABEL {
            return Err(AppearanceError::InvalidSpec(format!(
                "milky_way.label must be {MILKY_WAY_LABEL}"
            )));
        }
        let pole_n = norm3(self.milky_way.pole);
        if !pole_n.is_finite() || (pole_n - 1.0).abs() > 1e-9 {
            return Err(AppearanceError::InvalidSpec(
                "milky_way.pole must be unit".into(),
            ));
        }
        for (name, v) in [
            ("band_sigma_rad", self.milky_way.band_sigma_rad),
            ("core_sigma_rad", self.milky_way.core_sigma_rad),
            ("dust_sigma_rad", self.milky_way.dust_sigma_rad),
        ] {
            if !v.is_finite() || !(v > 0.0) {
                return Err(AppearanceError::InvalidSpec(format!(
                    "{name} must be finite and > 0"
                )));
            }
        }
        for (name, v) in [
            ("band_peak", self.milky_way.band_peak),
            ("core_peak", self.milky_way.core_peak),
            ("dust_depth", self.milky_way.dust_depth),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(AppearanceError::InvalidSpec(format!(
                    "{name} must be finite and >= 0"
                )));
            }
        }
        if !(0.0..=1.0).contains(&self.milky_way.longitude_modulation_amp)
            || !self.milky_way.longitude_modulation_amp.is_finite()
        {
            return Err(AppearanceError::InvalidSpec(
                "longitude_modulation_amp in [0,1]".into(),
            ));
        }
        if self.stars.profile_id != STAR_PROFILE_ID {
            return Err(AppearanceError::InvalidSpec(format!(
                "stars.profile_id must be {STAR_PROFILE_ID}"
            )));
        }
        if self.stars.algorithm_id != HASH_PRNG_ID {
            return Err(AppearanceError::InvalidSpec(format!(
                "stars.algorithm_id must be {HASH_PRNG_ID}"
            )));
        }
        if !self.stars.angular_sigma_rad.is_finite() || !(self.stars.angular_sigma_rad > 0.0) {
            return Err(AppearanceError::InvalidSpec(
                "angular_sigma_rad must be finite and > 0 (A1)".into(),
            ));
        }
        if !self.stars.peak_scale.is_finite() || self.stars.peak_scale < 0.0 {
            return Err(AppearanceError::InvalidSpec(
                "peak_scale must be finite and >= 0".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.stars.band_bias) {
            return Err(AppearanceError::InvalidSpec(
                "band_bias must be in [0,1]".into(),
            ));
        }
        if !(self.stars.t_min_k.is_finite()
            && self.stars.t_max_k.is_finite()
            && self.stars.t_min_k > 0.0
            && self.stars.t_max_k > self.stars.t_min_k)
        {
            return Err(AppearanceError::InvalidSpec(
                "star temperature range invalid".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProceduralStar {
    pub direction: [f64; 3],
    pub peak: f64,
    pub rgb_chromaticity: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct CelestialEnvironment {
    pub spec: EnvironmentSpec,
    pub stars: Vec<ProceduralStar>,
    pub cells: Vec<Vec<usize>>,
    pub n_lat: usize,
    pub n_lon: usize,
}

fn norm3(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn normalize3(v: [f64; 3]) -> Result<[f64; 3], AppearanceError> {
    let n = norm3(v);
    if !n.is_finite() || n == 0.0 {
        return Err(AppearanceError::NonFinite("normalize3".into()));
    }
    Ok([v[0] / n, v[1] / n, v[2] / n])
}

/// Project-owned splitmix64 (Steele / public-domain family; frozen constants).
pub fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn u01(seed: u64, stream: u64) -> f64 {
    let v = splitmix64(seed ^ stream.wrapping_mul(0xD6E8_FEB8_66D5_CBAA));
    (v >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
}

fn approx_blackbody_chromaticity(t_k: f64) -> [f64; 3] {
    let t = t_k.clamp(2500.0, 15000.0);
    let (r, g, b) = if t < 4000.0 {
        let u = (t - 2500.0) / 1500.0;
        (1.0, 0.35 + 0.35 * u, 0.08 + 0.12 * u)
    } else if t < 6500.0 {
        let u = (t - 4000.0) / 2500.0;
        (1.0, 0.70 + 0.20 * u, 0.20 + 0.55 * u)
    } else {
        let u = (t - 6500.0) / 8500.0;
        (1.0 - 0.35 * u, 0.90 - 0.05 * u, 1.0)
    };
    let s = r + g + b;
    [r / s, g / s, b / s]
}

fn sample_direction_biased(
    seed: u64,
    i: u64,
    pole: [f64; 3],
    band_bias: f64,
) -> Result<[f64; 3], AppearanceError> {
    let u1 = u01(seed, i.wrapping_mul(3) + 1);
    let u2 = u01(seed, i.wrapping_mul(3) + 2);
    let z = 2.0 * u1 - 1.0;
    let phi = std::f64::consts::TAU * u2;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let mut d = [r * phi.cos(), r * phi.sin(), z];
    let pull = u01(seed, i.wrapping_mul(3) + 3);
    if pull < band_bias {
        let proj = d[0] * pole[0] + d[1] * pole[1] + d[2] * pole[2];
        d = [
            d[0] - 0.85 * proj * pole[0],
            d[1] - 0.85 * proj * pole[1],
            d[2] - 0.85 * proj * pole[2],
        ];
    }
    normalize3(d)
}

pub fn environment_spec_digest(spec: &EnvironmentSpec) -> Result<String, AppearanceError> {
    spec.validate()?;
    let mut h = Sha256::new();
    h.update(b"environment-spec-digest-v1");
    h.update(b"APPEARANCE_REPRODUCIBILITY_DIGEST");
    h.update(spec.model_id.as_bytes());
    h.update([u8::from(spec.identity_black)]);
    h.update(spec.sky_floor.to_bits().to_le_bytes());
    let q = &spec.environment_rotation;
    h.update(q.w.to_bits().to_le_bytes());
    h.update(q.x.to_bits().to_le_bytes());
    h.update(q.y.to_bits().to_le_bytes());
    h.update(q.z.to_bits().to_le_bytes());
    let mw = &spec.milky_way;
    h.update(mw.label.as_bytes());
    for c in mw.pole {
        h.update(c.to_bits().to_le_bytes());
    }
    h.update(mw.band_sigma_rad.to_bits().to_le_bytes());
    h.update(mw.band_peak.to_bits().to_le_bytes());
    h.update(mw.core_sigma_rad.to_bits().to_le_bytes());
    h.update(mw.core_peak.to_bits().to_le_bytes());
    h.update(mw.dust_sigma_rad.to_bits().to_le_bytes());
    h.update(mw.dust_depth.to_bits().to_le_bytes());
    h.update(mw.longitude_modulation_amp.to_bits().to_le_bytes());
    h.update(mw.longitude_harmonics.to_le_bytes());
    let st = &spec.stars;
    h.update(st.profile_id.as_bytes());
    h.update(st.algorithm_id.as_bytes());
    h.update(st.seed.to_le_bytes());
    h.update(st.count.to_le_bytes());
    h.update(st.angular_sigma_rad.to_bits().to_le_bytes());
    h.update(st.peak_scale.to_bits().to_le_bytes());
    h.update(st.band_bias.to_bits().to_le_bytes());
    h.update(st.t_min_k.to_bits().to_le_bytes());
    h.update(st.t_max_k.to_bits().to_le_bytes());
    Ok(hex_sha(&h.finalize()))
}

pub fn build_celestial_environment(
    spec: &EnvironmentSpec,
) -> Result<CelestialEnvironment, AppearanceError> {
    spec.validate()?;
    if spec.identity_black {
        return Ok(CelestialEnvironment {
            spec: spec.clone(),
            stars: Vec::new(),
            cells: Vec::new(),
            n_lat: 0,
            n_lon: 0,
        });
    }
    let pole = normalize3(spec.milky_way.pole)?;
    let mut stars = Vec::with_capacity(spec.stars.count as usize);
    for i in 0..u64::from(spec.stars.count) {
        let dir = sample_direction_biased(spec.stars.seed, i, pole, spec.stars.band_bias)?;
        let u_t = u01(spec.stars.seed, i.wrapping_mul(7) + 11);
        let t = spec.stars.t_min_k + u_t * (spec.stars.t_max_k - spec.stars.t_min_k);
        let chroma = approx_blackbody_chromaticity(t);
        let u_p = u01(spec.stars.seed, i.wrapping_mul(7) + 13);
        let peak = spec.stars.peak_scale * (0.15 + 0.85 * u_p.powf(2.5));
        stars.push(ProceduralStar {
            direction: dir,
            peak,
            rgb_chromaticity: chroma,
        });
    }

    let n_lat = 24usize;
    let n_lon = 48usize;
    let mut cells = vec![Vec::new(); n_lat * n_lon];
    for (idx, star) in stars.iter().enumerate() {
        let (lat, lon) = dir_to_lat_lon(star.direction);
        let ilat = ((lat / std::f64::consts::PI) * n_lat as f64).floor() as usize;
        let ilon = ((lon / std::f64::consts::TAU) * n_lon as f64).floor() as usize;
        let ilat = ilat.min(n_lat - 1);
        let ilon = ilon.min(n_lon - 1);
        cells[ilat * n_lon + ilon].push(idx);
    }

    Ok(CelestialEnvironment {
        spec: spec.clone(),
        stars,
        cells,
        n_lat,
        n_lon,
    })
}

fn dir_to_lat_lon(d: [f64; 3]) -> (f64, f64) {
    let lat = d[2].clamp(-1.0, 1.0).acos();
    let lon = d[1].atan2(d[0]).rem_euclid(std::f64::consts::TAU);
    (lat, lon)
}

fn milky_way_component(mw: &MilkyWayLikeSpec, d: [f64; 3]) -> f64 {
    let pole = mw.pole;
    let cos_lat = (d[0] * pole[0] + d[1] * pole[1] + d[2] * pole[2]).clamp(-1.0, 1.0);
    let lat_from_eq = cos_lat.abs().asin();
    let band = (-0.5 * (lat_from_eq / mw.band_sigma_rad).powi(2)).exp();
    let core = (-0.5 * (lat_from_eq / mw.core_sigma_rad).powi(2)).exp();
    let dust = (-0.5 * (lat_from_eq / mw.dust_sigma_rad).powi(2)).exp();
    let proj = [
        d[0] - cos_lat * pole[0],
        d[1] - cos_lat * pole[1],
        d[2] - cos_lat * pole[2],
    ];
    let pn = norm3(proj);
    let lon = if pn > 1e-14 {
        let ax = if pole[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let e1 = normalize3([
            pole[1] * ax[2] - pole[2] * ax[1],
            pole[2] * ax[0] - pole[0] * ax[2],
            pole[0] * ax[1] - pole[1] * ax[0],
        ])
        .unwrap_or([1.0, 0.0, 0.0]);
        let e2 = [
            pole[1] * e1[2] - pole[2] * e1[1],
            pole[2] * e1[0] - pole[0] * e1[2],
            pole[0] * e1[1] - pole[1] * e1[0],
        ];
        let x = (proj[0] * e1[0] + proj[1] * e1[1] + proj[2] * e1[2]) / pn;
        let y = (proj[0] * e2[0] + proj[1] * e2[1] + proj[2] * e2[2]) / pn;
        y.atan2(x)
    } else {
        0.0
    };
    let harm = mw.longitude_harmonics.max(1);
    let lon_mod = 1.0 + mw.longitude_modulation_amp * (f64::from(harm) * lon).cos();
    let glow = mw.band_peak * band * lon_mod.max(0.0) + mw.core_peak * core;
    (glow * (1.0 - mw.dust_depth * dust).max(0.0)).max(0.0)
}

fn angular_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
    dot.acos()
}

fn star_contribution(env: &CelestialEnvironment, d: [f64; 3]) -> Result<[f64; 3], AppearanceError> {
    let sigma = env.spec.stars.angular_sigma_rad;
    let support = 3.0 * sigma;
    let (lat, lon) = dir_to_lat_lon(d);
    let mut rgb = [0.0; 3];
    if env.n_lat == 0 {
        return Ok(rgb);
    }
    let lat0 =
        ((lat - support).max(0.0) / std::f64::consts::PI * env.n_lat as f64).floor() as isize;
    let lat1 = ((lat + support).min(std::f64::consts::PI) / std::f64::consts::PI * env.n_lat as f64)
        .floor() as isize;
    let dlon = support / lat.sin().max(0.15);
    let lon0 = ((lon - dlon) / std::f64::consts::TAU * env.n_lon as f64).floor() as isize;
    let lon1 = ((lon + dlon) / std::f64::consts::TAU * env.n_lon as f64).floor() as isize;
    for ilat in lat0..=lat1 {
        if ilat < 0 || ilat >= env.n_lat as isize {
            continue;
        }
        for ilon in lon0..=lon1 {
            let wrapped = ilon.rem_euclid(env.n_lon as isize) as usize;
            for &idx in &env.cells[ilat as usize * env.n_lon + wrapped] {
                let star = &env.stars[idx];
                let ang = angular_distance(d, star.direction);
                if ang > support {
                    continue;
                }
                let w = (-0.5 * (ang / sigma).powi(2)).exp();
                let i = star.peak * w;
                rgb[0] += i * star.rgb_chromaticity[0];
                rgb[1] += i * star.rgb_chromaticity[1];
                rgb[2] += i * star.rgb_chromaticity[2];
            }
        }
    }
    if !rgb.iter().all(|c| c.is_finite() && *c >= 0.0) {
        return Err(AppearanceError::NonFinite("star rgb".into()));
    }
    Ok(rgb)
}

/// Sample environment in middle-gray-relative appearance-linear Rec.709 (S2).
pub fn sample_environment_linear(
    env: &CelestialEnvironment,
    sample: &CelestialBoundarySample,
) -> Result<LinearRgb, AppearanceError> {
    if env.spec.identity_black {
        return LinearRgb::new(0.0, 0.0, 0.0)
            .map_err(|e| AppearanceError::Presentation(e.to_string()));
    }
    let d0 = sample.unit_coordinate_direction;
    if !d0.iter().all(|c| c.is_finite()) {
        return Err(AppearanceError::NonFinite(
            "unit_coordinate_direction".into(),
        ));
    }
    let d = normalize3(env.spec.environment_rotation.rotate_vector(d0))?;
    let floor = env.spec.sky_floor;
    let mw = milky_way_component(&env.spec.milky_way, d);
    let level = floor + mw;
    let band_rgb = [0.55 * level, 0.62 * level, 0.85 * level];
    let stars = star_contribution(env, d)?;
    let r = band_rgb[0] + stars[0];
    let g = band_rgb[1] + stars[1];
    let b = band_rgb[2] + stars[2];
    if !(r.is_finite() && g.is_finite() && b.is_finite()) || r < 0.0 || g < 0.0 || b < 0.0 {
        return Err(AppearanceError::NonFinite(format!("env rgb ({r},{g},{b})")));
    }
    LinearRgb::new(r, g, b).map_err(|e| AppearanceError::Presentation(e.to_string()))
}

/// Unlensed environment preview (diagnostic).
pub fn render_environment_reference(
    env: &CelestialEnvironment,
    width: u32,
    height: u32,
) -> Result<Vec<LinearRgb>, AppearanceError> {
    let mut out = Vec::with_capacity((width * height) as usize);
    for row in 0..height {
        let v = (f64::from(row) + 0.5) / f64::from(height);
        let theta = v * std::f64::consts::PI;
        for col in 0..width {
            let u = (f64::from(col) + 0.5) / f64::from(width);
            let psi = u * std::f64::consts::TAU;
            let dir = [
                theta.sin() * psi.cos(),
                theta.sin() * psi.sin(),
                theta.cos(),
            ];
            let sample = CelestialBoundarySample {
                source: CelestialDirectionSource::FiniteOblateEscapeBoundaryPosition,
                oblate_radius: 80.0,
                theta,
                psi,
                unit_coordinate_direction: dir,
                uv: CelestialUv { u, v },
                azimuth_status: SphericalKsAzimuthStatus::Defined,
                escape_event_value: 0.0,
            };
            out.push(sample_environment_linear(env, &sample)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_spec(identity: bool) -> EnvironmentSpec {
        EnvironmentSpec {
            model_id: ENVIRONMENT_MODEL_ID.into(),
            environment_rotation: UnitQuaternion::identity(),
            sky_floor: 0.002,
            milky_way: MilkyWayLikeSpec {
                label: MILKY_WAY_LABEL.into(),
                pole: [0.0, 0.0, 1.0],
                band_sigma_rad: 0.25,
                band_peak: 0.04,
                core_sigma_rad: 0.08,
                core_peak: 0.06,
                dust_sigma_rad: 0.05,
                dust_depth: 0.35,
                longitude_modulation_amp: 0.25,
                longitude_harmonics: 2,
            },
            stars: StarsSpec {
                profile_id: STAR_PROFILE_ID.into(),
                seed: 0x002d_1ace,
                algorithm_id: HASH_PRNG_ID.into(),
                count: 8,
                angular_sigma_rad: 0.015,
                peak_scale: 0.8,
                band_bias: 0.55,
                t_min_k: 3500.0,
                t_max_k: 12000.0,
            },
            identity_black: identity,
        }
    }

    #[test]
    fn quaternion_identity_rotation() {
        let q = UnitQuaternion::identity();
        let v = [0.3, -0.4, 0.866_025_403_784_438_6];
        let o = q.rotate_vector(v);
        for i in 0..3 {
            assert!((o[i] - v[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn deterministic_catalog_and_sample() {
        let env = build_celestial_environment(&tiny_spec(false)).unwrap();
        assert_eq!(env.stars.len(), 8);
        let env2 = build_celestial_environment(&tiny_spec(false)).unwrap();
        assert_eq!(env.stars[0].direction, env2.stars[0].direction);
        let s = CelestialBoundarySample {
            source: CelestialDirectionSource::FiniteOblateEscapeBoundaryPosition,
            oblate_radius: 80.0,
            theta: std::f64::consts::FRAC_PI_2,
            psi: 0.0,
            unit_coordinate_direction: [1.0, 0.0, 0.0],
            uv: CelestialUv { u: 0.1, v: 0.5 },
            azimuth_status: SphericalKsAzimuthStatus::Defined,
            escape_event_value: 0.0,
        };
        let a = sample_environment_linear(&env, &s).unwrap();
        let b = sample_environment_linear(&env2, &s).unwrap();
        assert_eq!(a, b);
        assert!(a.r >= 0.0 && a.g >= 0.0 && a.b >= 0.0);
    }

    #[test]
    fn identity_black() {
        let env = build_celestial_environment(&tiny_spec(true)).unwrap();
        let s = CelestialBoundarySample {
            source: CelestialDirectionSource::FiniteOblateEscapeBoundaryPosition,
            oblate_radius: 80.0,
            theta: std::f64::consts::FRAC_PI_2,
            psi: 0.0,
            unit_coordinate_direction: [0.0, 1.0, 0.0],
            uv: CelestialUv { u: 0.0, v: 0.0 },
            azimuth_status: SphericalKsAzimuthStatus::Defined,
            escape_event_value: 0.0,
        };
        let c = sample_environment_linear(&env, &s).unwrap();
        assert_eq!(c.r, 0.0);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);
    }
}
