//! Procedural celestial texture V1 — diagnostic coordinate/orientation field.
//!
//! Artistic diagnostic parameters (documented, not physical):
//! - `MINOR_LINE_HALF_WIDTH_CELL = 0.06` of a texture cell
//! - `MAJOR_LINE_HALF_WIDTH_CELL = 0.10` of a texture cell
//! - `EQUATOR_HALF_WIDTH_V = 0.004` in v
//! - `SEAM_HALF_WIDTH_U = 0.003` in u (wraps across 0/1)

use crate::error::CelestialRenderError;
use relativity_trace::{hex_sha, CelestialBoundarySample, CelestialUv, RgbFrame, TraceGrid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TEXTURE_ID_V1: &str = "procedural-coordinate-grid-v1";

/// Artistic diagnostic line half-widths (cell units for grid; UV for equator/seam).
pub const MINOR_LINE_HALF_WIDTH_CELL: f64 = 0.06;
pub const MAJOR_LINE_HALF_WIDTH_CELL: f64 = 0.10;
pub const EQUATOR_HALF_WIDTH_V: f64 = 0.004;
pub const SEAM_HALF_WIDTH_U: f64 = 0.003;

const NORTH_PALETTE: [[u8; 3]; 8] = [
    [28, 72, 160],
    [24, 120, 142],
    [40, 132, 76],
    [152, 132, 36],
    [164, 72, 36],
    [148, 48, 132],
    [84, 60, 168],
    [40, 104, 176],
];

const SOUTH_PALETTE: [[u8; 3]; 8] = [
    [16, 40, 96],
    [16, 72, 84],
    [24, 80, 48],
    [92, 80, 24],
    [100, 44, 24],
    [88, 28, 80],
    [48, 36, 104],
    [24, 60, 108],
];

const COLOR_MINOR_GRID: [u8; 3] = [112, 112, 112];
const COLOR_MAJOR_GRID: [u8; 3] = [232, 232, 232];
const COLOR_EQUATOR: [u8; 3] = [255, 255, 255];
const COLOR_SEAM: [u8; 3] = [255, 48, 48];

const MARKERS: [([f64; 3], [u8; 3]); 6] = [
    ([1.0, 0.0, 0.0], [255, 32, 32]),    // +X
    ([0.0, 1.0, 0.0], [32, 255, 64]),    // +Y
    ([-1.0, 0.0, 0.0], [255, 220, 32]),  // -X
    ([0.0, -1.0, 0.0], [32, 220, 255]),  // -Y
    ([0.0, 0.0, 1.0], [255, 32, 220]),   // north
    ([0.0, 0.0, -1.0], [255, 255, 255]), // south
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProceduralCelestialTextureSpec {
    pub schema_version: u32,
    pub texture_id: String,
    pub longitude_sectors: u32,
    pub latitude_cells: u32,
    pub minor_longitude_divisions: u32,
    pub minor_latitude_divisions: u32,
    pub major_longitude_stride: u32,
    pub major_latitude_stride: u32,
    pub marker_radius_millidegrees: u32,
}

pub fn procedural_coordinate_grid_v1() -> ProceduralCelestialTextureSpec {
    ProceduralCelestialTextureSpec {
        schema_version: 1,
        texture_id: TEXTURE_ID_V1.into(),
        longitude_sectors: 8,
        latitude_cells: 12,
        minor_longitude_divisions: 24,
        minor_latitude_divisions: 12,
        major_longitude_stride: 3,
        major_latitude_stride: 3,
        marker_radius_millidegrees: 7000,
    }
}

impl ProceduralCelestialTextureSpec {
    pub fn validate(&self) -> Result<(), CelestialRenderError> {
        if self.schema_version != 1 {
            return Err(CelestialRenderError::InvalidTextureSpec(
                "schema_version must be 1".into(),
            ));
        }
        if self.texture_id != TEXTURE_ID_V1 {
            return Err(CelestialRenderError::UnsupportedTextureId(
                self.texture_id.clone(),
            ));
        }
        for (name, v) in [
            ("longitude_sectors", self.longitude_sectors),
            ("latitude_cells", self.latitude_cells),
            ("minor_longitude_divisions", self.minor_longitude_divisions),
            ("minor_latitude_divisions", self.minor_latitude_divisions),
            ("major_longitude_stride", self.major_longitude_stride),
            ("major_latitude_stride", self.major_latitude_stride),
            (
                "marker_radius_millidegrees",
                self.marker_radius_millidegrees,
            ),
        ] {
            if v == 0 {
                return Err(CelestialRenderError::InvalidTextureSpec(format!(
                    "{name} must be > 0"
                )));
            }
        }
        if !self
            .minor_longitude_divisions
            .is_multiple_of(self.longitude_sectors)
        {
            return Err(CelestialRenderError::InvalidTextureSpec(
                "minor_longitude_divisions must be multiple of longitude_sectors".into(),
            ));
        }
        if self.major_longitude_stride > self.minor_longitude_divisions
            || self.major_latitude_stride > self.minor_latitude_divisions
        {
            return Err(CelestialRenderError::InvalidTextureSpec(
                "major stride exceeds minor divisions".into(),
            ));
        }
        Ok(())
    }
}

pub fn procedural_texture_spec_digest(spec: &ProceduralCelestialTextureSpec) -> String {
    let mut h = Sha256::new();
    update_tagged_bytes(&mut h, b"domain", b"procedural-texture-spec-digest-v1");
    h.update(spec.schema_version.to_le_bytes());
    update_tagged_str(&mut h, b"texture-id", &spec.texture_id);
    h.update(spec.longitude_sectors.to_le_bytes());
    h.update(spec.latitude_cells.to_le_bytes());
    h.update(spec.minor_longitude_divisions.to_le_bytes());
    h.update(spec.minor_latitude_divisions.to_le_bytes());
    h.update(spec.major_longitude_stride.to_le_bytes());
    h.update(spec.major_latitude_stride.to_le_bytes());
    h.update(spec.marker_radius_millidegrees.to_le_bytes());
    // Artistic line-width constants are part of the frozen visual contract.
    update_tagged_bytes(
        &mut h,
        b"minor-line-half-width-cell-bits",
        &MINOR_LINE_HALF_WIDTH_CELL.to_bits().to_le_bytes(),
    );
    update_tagged_bytes(
        &mut h,
        b"major-line-half-width-cell-bits",
        &MAJOR_LINE_HALF_WIDTH_CELL.to_bits().to_le_bytes(),
    );
    update_tagged_bytes(
        &mut h,
        b"equator-half-width-v-bits",
        &EQUATOR_HALF_WIDTH_V.to_bits().to_le_bytes(),
    );
    update_tagged_bytes(
        &mut h,
        b"seam-half-width-u-bits",
        &SEAM_HALF_WIDTH_U.to_bits().to_le_bytes(),
    );
    hex_sha(&h.finalize())
}

fn update_tagged_str(h: &mut Sha256, tag: &[u8], value: &str) {
    update_tagged_bytes(h, tag, value.as_bytes());
}

fn update_tagged_bytes(h: &mut Sha256, tag: &[u8], value: &[u8]) {
    h.update((tag.len() as u64).to_le_bytes());
    h.update(tag);
    h.update((value.len() as u64).to_le_bytes());
    h.update(value);
}

fn saturating_add_rgb(base: [u8; 3], delta: u8) -> [u8; 3] {
    [
        base[0].saturating_add(delta),
        base[1].saturating_add(delta),
        base[2].saturating_add(delta),
    ]
}

fn wrap_dist_u(u: f64, target: f64) -> f64 {
    let d = (u - target).abs();
    d.min(1.0 - d)
}

fn cell_boundary_dist(coord: f64) -> f64 {
    // coord in cell units; distance to nearest integer boundary in [0, 0.5]
    let f = coord - coord.floor();
    f.min(1.0 - f)
}

fn sample_uv_direction(
    spec: &ProceduralCelestialTextureSpec,
    uv: CelestialUv,
    direction: [f64; 3],
) -> Result<[u8; 3], CelestialRenderError> {
    spec.validate()?;
    let u = uv.u;
    let v = uv.v;
    if !(0.0..1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return Err(CelestialRenderError::InvalidSample(format!("uv=({u},{v})")));
    }
    if !direction.iter().all(|c| c.is_finite()) {
        return Err(CelestialRenderError::InvalidSample(
            "non-finite direction".into(),
        ));
    }

    // Marker override (fixed priority order).
    let marker_rad =
        (spec.marker_radius_millidegrees as f64) * 1.0e-3 * std::f64::consts::PI / 180.0;
    for (mdir, color) in MARKERS {
        let dot = direction[0] * mdir[0] + direction[1] * mdir[1] + direction[2] * mdir[2];
        let angle = dot.clamp(-1.0, 1.0).acos();
        if angle <= marker_rad {
            return Ok(color);
        }
    }

    // Seam / equator (above major grid).
    if wrap_dist_u(u, 0.0) <= SEAM_HALF_WIDTH_U {
        return Ok(COLOR_SEAM);
    }
    if (v - 0.5).abs() <= EQUATOR_HALF_WIDTH_V {
        return Ok(COLOR_EQUATOR);
    }

    let lon_div = spec.minor_longitude_divisions as f64;
    let lat_div = spec.minor_latitude_divisions as f64;
    let lon_cell_f = u * lon_div;
    let lat_cell_f = (v * lat_div).min(lat_div - 1e-12);
    let d_lon = cell_boundary_dist(lon_cell_f);
    let d_lat = cell_boundary_dist(lat_cell_f);

    // Major lines: distance to nearest major-stride boundary, measured in minor-cell units.
    let major_lon_period = spec.major_longitude_stride as f64;
    let major_lat_period = spec.major_latitude_stride as f64;
    let on_major = cell_boundary_dist(lon_cell_f / major_lon_period) * major_lon_period
        <= MAJOR_LINE_HALF_WIDTH_CELL
        || cell_boundary_dist(lat_cell_f / major_lat_period) * major_lat_period
            <= MAJOR_LINE_HALF_WIDTH_CELL;

    let on_minor = d_lon <= MINOR_LINE_HALF_WIDTH_CELL || d_lat <= MINOR_LINE_HALF_WIDTH_CELL;

    if on_major {
        return Ok(COLOR_MAJOR_GRID);
    }
    if on_minor {
        return Ok(COLOR_MINOR_GRID);
    }

    // Base sector + checker
    let sector = ((u * spec.longitude_sectors as f64).floor() as u32) % spec.longitude_sectors;
    let base = if v < 0.5 {
        NORTH_PALETTE[sector as usize]
    } else {
        SOUTH_PALETTE[sector as usize]
    };
    let lon_cell = ((u * spec.minor_longitude_divisions as f64).floor() as u32)
        % spec.minor_longitude_divisions;
    let lat_cell = ((v * spec.minor_latitude_divisions as f64).floor() as u32)
        .min(spec.minor_latitude_divisions - 1);
    let checker = (lon_cell + lat_cell) % 2 == 1;
    Ok(if checker {
        saturating_add_rgb(base, 18)
    } else {
        base
    })
}

pub fn sample_procedural_celestial(
    spec: &ProceduralCelestialTextureSpec,
    sample: &CelestialBoundarySample,
) -> Result<[u8; 3], CelestialRenderError> {
    sample_uv_direction(spec, sample.uv, sample.unit_coordinate_direction)
}

pub fn render_procedural_texture_reference(
    spec: &ProceduralCelestialTextureSpec,
    width: u32,
    height: u32,
) -> Result<RgbFrame, CelestialRenderError> {
    spec.validate()?;
    if width == 0 || height == 0 {
        return Err(CelestialRenderError::ZeroDimensions);
    }
    let mut pixels = Vec::with_capacity((width as usize) * (height as usize));
    for row in 0..height {
        for col in 0..width {
            let u = (col as f64 + 0.5) / width as f64;
            let v = (row as f64 + 0.5) / height as f64;
            let theta = std::f64::consts::PI * v;
            let psi = std::f64::consts::TAU * u;
            let direction = [
                theta.sin() * psi.cos(),
                theta.sin() * psi.sin(),
                theta.cos(),
            ];
            let rgb = sample_uv_direction(spec, CelestialUv { u, v }, direction)?;
            pixels.push(rgb);
        }
    }
    RgbFrame::try_new(TraceGrid { width, height }, pixels)
        .map_err(|_| CelestialRenderError::FrameLengthMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_core::SphericalKsAzimuthStatus;
    use relativity_trace::CelestialDirectionSource;

    fn sample_at(u: f64, v: f64) -> CelestialBoundarySample {
        let theta = std::f64::consts::PI * v;
        let psi = std::f64::consts::TAU * u;
        CelestialBoundarySample {
            source: CelestialDirectionSource::FiniteOblateEscapeBoundaryPosition,
            oblate_radius: 80.0,
            theta,
            psi,
            unit_coordinate_direction: [
                theta.sin() * psi.cos(),
                theta.sin() * psi.sin(),
                theta.cos(),
            ],
            uv: CelestialUv { u, v },
            azimuth_status: SphericalKsAzimuthStatus::Defined,
            escape_event_value: 0.0,
        }
    }

    #[test]
    fn cardinal_markers_exact_colors() {
        let spec = procedural_coordinate_grid_v1();
        // +X at equator: u=0, v=0.5
        assert_eq!(
            sample_procedural_celestial(&spec, &sample_at(0.0, 0.5)).unwrap(),
            [255, 32, 32]
        );
        // +Y: u=0.25
        assert_eq!(
            sample_procedural_celestial(&spec, &sample_at(0.25, 0.5)).unwrap(),
            [32, 255, 64]
        );
        // -X: u=0.5
        assert_eq!(
            sample_procedural_celestial(&spec, &sample_at(0.5, 0.5)).unwrap(),
            [255, 220, 32]
        );
        // -Y: u=0.75
        assert_eq!(
            sample_procedural_celestial(&spec, &sample_at(0.75, 0.5)).unwrap(),
            [32, 220, 255]
        );
        // north pole
        assert_eq!(
            sample_procedural_celestial(&spec, &sample_at(0.0, 0.0)).unwrap(),
            [255, 32, 220]
        );
        // south pole
        assert_eq!(
            sample_procedural_celestial(&spec, &sample_at(0.0, 1.0)).unwrap(),
            [255, 255, 255]
        );
    }

    #[test]
    fn north_south_palettes_differ() {
        let spec = procedural_coordinate_grid_v1();
        // Interior of sector-0 / lon_cell=1 / lat cells away from grid & markers.
        let n = sample_procedural_celestial(&spec, &sample_at(1.5 / 24.0, 2.5 / 12.0)).unwrap();
        let s = sample_procedural_celestial(&spec, &sample_at(1.5 / 24.0, 8.5 / 12.0)).unwrap();
        assert_eq!(n, saturating_add_rgb(NORTH_PALETTE[0], 18));
        assert_eq!(s, saturating_add_rgb(SOUTH_PALETTE[0], 18));
        assert_ne!(n, s);
    }

    #[test]
    fn checker_parity_deterministic() {
        let spec = procedural_coordinate_grid_v1();
        // Interior of lon_cell=1, lat_cell=0 → checker on (+18)
        let a = sample_procedural_celestial(&spec, &sample_at(1.5 / 24.0, 0.5 / 12.0)).unwrap();
        let b = sample_procedural_celestial(&spec, &sample_at(1.5 / 24.0, 0.5 / 12.0)).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, saturating_add_rgb(NORTH_PALETTE[0], 18));
        // Adjacent lon cell even → base
        let c = sample_procedural_celestial(&spec, &sample_at(0.5 / 24.0, 0.5 / 12.0)).unwrap();
        assert_eq!(c, NORTH_PALETTE[0]);
    }

    #[test]
    fn seam_and_equator_colors() {
        let spec = procedural_coordinate_grid_v1();
        // Near seam but not on +X marker center — use tiny offset from u=0 that's still seam
        // Actually +X marker covers seam at equator. Use mid-latitude near seam.
        let seam = sample_procedural_celestial(&spec, &sample_at(0.001, 0.3)).unwrap();
        assert_eq!(seam, COLOR_SEAM);
        // Equator away from markers (u=0.125 is between +X and +Y)
        let eq = sample_procedural_celestial(&spec, &sample_at(0.125, 0.5)).unwrap();
        assert_eq!(eq, COLOR_EQUATOR);
    }

    #[test]
    fn sample_deterministic() {
        let spec = procedural_coordinate_grid_v1();
        let s = sample_at(0.33, 0.4);
        let a = sample_procedural_celestial(&spec, &s).unwrap();
        let b = sample_procedural_celestial(&spec, &s).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn invalid_spec_rejected() {
        let mut s = procedural_coordinate_grid_v1();
        s.minor_longitude_divisions = 0;
        assert!(s.validate().is_err());
        s = procedural_coordinate_grid_v1();
        s.texture_id = "other".into();
        assert!(matches!(
            s.validate(),
            Err(CelestialRenderError::UnsupportedTextureId(_))
        ));
    }

    #[test]
    fn every_spec_field_affects_digest() {
        let base = procedural_coordinate_grid_v1();
        let d0 = procedural_texture_spec_digest(&base);
        let mut s = base.clone();
        s.longitude_sectors = 4;
        // may fail validate but digest still hashes fields
        assert_ne!(d0, procedural_texture_spec_digest(&s));
        s = base.clone();
        s.latitude_cells = 6;
        assert_ne!(d0, procedural_texture_spec_digest(&s));
        s = base.clone();
        s.minor_longitude_divisions = 16;
        assert_ne!(d0, procedural_texture_spec_digest(&s));
        s = base.clone();
        s.minor_latitude_divisions = 6;
        assert_ne!(d0, procedural_texture_spec_digest(&s));
        s = base.clone();
        s.major_longitude_stride = 2;
        assert_ne!(d0, procedural_texture_spec_digest(&s));
        s = base.clone();
        s.major_latitude_stride = 2;
        assert_ne!(d0, procedural_texture_spec_digest(&s));
        s = base.clone();
        s.marker_radius_millidegrees = 5000;
        assert_ne!(d0, procedural_texture_spec_digest(&s));
        s = base.clone();
        s.texture_id = "procedural-coordinate-grid-v1x".into();
        assert_ne!(d0, procedural_texture_spec_digest(&s));
    }

    #[test]
    fn reference_atlas_row_major_deterministic() {
        let spec = procedural_coordinate_grid_v1();
        let a = render_procedural_texture_reference(&spec, 32, 16).unwrap();
        let b = render_procedural_texture_reference(&spec, 32, 16).unwrap();
        assert_eq!(a.pixels(), b.pixels());
        assert_eq!(a.grid().width, 32);
        assert_eq!(a.pixels().len(), 32 * 16);
    }
}
