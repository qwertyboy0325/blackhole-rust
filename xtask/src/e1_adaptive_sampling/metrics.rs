//! Candidate-vs-oracle metrics (post-reconstruction only).

use crate::e1_adaptive_sampling::reconstruct::AdaptiveReconstruction;
use crate::e1_adaptive_sampling::sample::AdaptiveRaySample;
use crate::oracle_benchmark::{compare_rgb, RgbComparisonMetrics};
use relativity_oracle::{
    IntegerErrorMetrics, OptionalScalarErrorMetrics, OracleComparisonMetrics, OracleFrame,
    OraclePixel, ScalarErrorMetrics,
};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleParityReport {
    pub selected_sample_count: u64,
    pub selected_sample_exact_count: u64,
    pub selected_sample_mismatch_count: u64,
}

pub fn verify_selected_sample_parity(
    samples: &[&AdaptiveRaySample],
    oracle: &OracleFrame,
    reference_ppm: &[u8],
) -> Result<SampleParityReport, Box<dyn Error>> {
    oracle.validate().map_err(|e| e.to_string())?;
    let mut exact = 0u64;
    let mut mismatch = 0u64;
    for s in samples {
        let op = oracle_pixel_by_source(oracle, s.source_index)
            .ok_or_else(|| format!("oracle missing source_index {}", s.source_index))?;
        let rgb_ok = reference_rgb_at(reference_ppm, oracle.width, op.col, op.row)
            .is_ok_and(|rgb| rgb == s.rgb);
        if sample_matches_oracle(s, op) && rgb_ok {
            exact += 1;
        } else {
            mismatch += 1;
        }
    }
    Ok(SampleParityReport {
        selected_sample_count: samples.len() as u64,
        selected_sample_exact_count: exact,
        selected_sample_mismatch_count: mismatch,
    })
}

fn reference_rgb_at(ppm: &[u8], width: u32, col: u32, row: u32) -> Result<[u8; 3], Box<dyn Error>> {
    let mut newlines = 0usize;
    let mut payload = 0usize;
    for (idx, b) in ppm.iter().enumerate() {
        if *b == b'\n' {
            newlines += 1;
            if newlines == 3 {
                payload = idx + 1;
                break;
            }
        }
    }
    let i = payload + ((row * width + col) * 3) as usize;
    if i + 2 >= ppm.len() {
        return Err("ppm index OOB".into());
    }
    Ok([ppm[i], ppm[i + 1], ppm[i + 2]])
}

fn oracle_pixel_by_source(oracle: &OracleFrame, source_index: u64) -> Option<&OraclePixel> {
    oracle
        .pixels
        .iter()
        .find(|p| p.source_index == source_index)
}

fn sample_matches_oracle(s: &AdaptiveRaySample, o: &OraclePixel) -> bool {
    if s.outcome_class != o.outcome_class || s.rhs_evaluations != o.rhs_evaluations {
        return false;
    }
    match (&s.celestial, &o.celestial) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            if a.theta.to_bits() != b.theta.to_bits()
                || a.psi.to_bits() != b.psi.to_bits()
                || a.u.to_bits() != b.u.to_bits()
                || a.v.to_bits() != b.v.to_bits()
                || a.direction[0].to_bits() != b.unit_coordinate_direction[0].to_bits()
                || a.direction[1].to_bits() != b.unit_coordinate_direction[1].to_bits()
                || a.direction[2].to_bits() != b.unit_coordinate_direction[2].to_bits()
            {
                return false;
            }
        }
        _ => return false,
    }
    match (&s.disk, &o.disk) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            if a.g_factor.to_bits() != b.g_factor.to_bits()
                || a.emitted_bolometric_intensity.to_bits()
                    != b.emitted_bolometric_intensity.to_bits()
                || a.observed_bolometric_intensity.to_bits()
                    != b.observed_bolometric_intensity.to_bits()
            {
                return false;
            }
        }
        _ => return false,
    }
    true
}

#[derive(Default)]
struct Acc {
    count: u64,
    sum_abs: f64,
    sum_sq: f64,
    max_abs: f64,
    max_index: u64,
}

impl Acc {
    fn push(&mut self, idx: u64, v: f64) {
        self.count += 1;
        self.sum_abs += v;
        self.sum_sq += v * v;
        if v > self.max_abs || (v == self.max_abs && idx < self.max_index) {
            self.max_abs = v;
            self.max_index = idx;
        }
        if self.count == 1 {
            self.max_index = idx;
        }
    }
    fn scalar(self) -> OptionalScalarErrorMetrics {
        if self.count == 0 {
            None
        } else {
            Some(ScalarErrorMetrics {
                mae: self.sum_abs / self.count as f64,
                rmse: (self.sum_sq / self.count as f64).sqrt(),
                maximum_absolute_error: self.max_abs,
                maximum_error_index: self.max_index,
            })
        }
    }
    fn integer(self) -> IntegerErrorMetrics {
        IntegerErrorMetrics {
            mae: if self.count == 0 {
                0.0
            } else {
                self.sum_abs / self.count as f64
            },
            rmse: if self.count == 0 {
                0.0
            } else {
                (self.sum_sq / self.count as f64).sqrt()
            },
            maximum_absolute_error: self.max_abs as u64,
            maximum_error_index: self.max_index,
        }
    }
}

pub fn compare_reconstruction_to_oracle(
    oracle: &OracleFrame,
    recon: &AdaptiveReconstruction,
) -> Result<OracleComparisonMetrics, Box<dyn Error>> {
    oracle.validate().map_err(|e| e.to_string())?;
    if oracle.width != recon.width || oracle.height != recon.height {
        return Err("dimension mismatch".into());
    }
    if oracle.pixels.len() != recon.pixels.len() {
        return Err("pixel length mismatch".into());
    }

    let mut outcome_disagreement_count = 0u64;
    let mut rhs = Acc::default();
    let mut celestial_presence_mismatch_count = 0u64;
    let mut celestial_angle = Acc::default();
    let mut celestial_u = Acc::default();
    let mut celestial_v = Acc::default();
    let mut celestial_pair_count = 0u64;
    let mut disk_presence_mismatch_count = 0u64;
    let mut log2_g = Acc::default();
    let mut log2_emitted = Acc::default();
    let mut log2_observed = Acc::default();
    let mut disk_pair_count = 0u64;

    for (idx, (o, c)) in oracle.pixels.iter().zip(&recon.pixels).enumerate() {
        let idx = idx as u64;
        let compatible = o.outcome_class == c.outcome_class;
        if !compatible {
            outcome_disagreement_count += 1;
        }
        rhs.push(idx, o.rhs_evaluations.abs_diff(c.rhs_evaluations) as f64);

        match (&o.celestial, &c.celestial) {
            (Some(a), Some(b)) => {
                if compatible {
                    celestial_pair_count += 1;
                    let ang = if a.unit_coordinate_direction[0].to_bits()
                        == b.direction[0].to_bits()
                        && a.unit_coordinate_direction[1].to_bits() == b.direction[1].to_bits()
                        && a.unit_coordinate_direction[2].to_bits() == b.direction[2].to_bits()
                    {
                        0.0
                    } else {
                        let dot = a.unit_coordinate_direction[0] * b.direction[0]
                            + a.unit_coordinate_direction[1] * b.direction[1]
                            + a.unit_coordinate_direction[2] * b.direction[2];
                        dot.clamp(-1.0, 1.0).acos()
                    };
                    celestial_angle.push(idx, ang);
                    let du = (a.u - b.u).abs();
                    celestial_u.push(idx, du.min(1.0 - du));
                    celestial_v.push(idx, (a.v - b.v).abs());
                }
            }
            (None, None) => {}
            _ => celestial_presence_mismatch_count += 1,
        }

        match (&o.disk, &c.disk) {
            (Some(a), Some(b)) => {
                if compatible {
                    disk_pair_count += 1;
                    log2_g.push(idx, (b.g_factor.log2() - a.g_factor.log2()).abs());
                    log2_emitted.push(
                        idx,
                        (b.emitted_bolometric_intensity.log2()
                            - a.emitted_bolometric_intensity.log2())
                        .abs(),
                    );
                    log2_observed.push(
                        idx,
                        (b.observed_bolometric_intensity.log2()
                            - a.observed_bolometric_intensity.log2())
                        .abs(),
                    );
                }
            }
            (None, None) => {}
            _ => disk_presence_mismatch_count += 1,
        }
    }

    let compared = oracle.pixels.len() as u64;
    Ok(OracleComparisonMetrics {
        compared_pixels: compared,
        outcome_disagreement_count,
        outcome_disagreement_rate: outcome_disagreement_count as f64 / compared as f64,
        rhs_absolute_error: rhs.integer(),
        celestial_pair_count,
        celestial_presence_mismatch_count,
        celestial_angular_error_radians: celestial_angle.scalar(),
        celestial_wrap_u_error: celestial_u.scalar(),
        celestial_v_error: celestial_v.scalar(),
        disk_pair_count,
        disk_presence_mismatch_count,
        log2_g_error: log2_g.scalar(),
        log2_emitted_error: log2_emitted.scalar(),
        log2_observed_error: log2_observed.scalar(),
    })
}

pub fn compare_reconstruction_rgb(
    reference_ppm: &[u8],
    candidate_ppm: &[u8],
) -> Result<RgbComparisonMetrics, Box<dyn Error>> {
    compare_rgb(reference_ppm, candidate_ppm)
}

/// Expected unique ray count for a full-coverage final ladder point.
pub fn final_coverage_ray_count(is_crop: bool) -> u64 {
    if is_crop {
        4096
    } else {
        16384
    }
}

fn scalar_error_exact_zero(m: &ScalarErrorMetrics) -> bool {
    m.mae == 0.0 && m.rmse == 0.0 && m.maximum_absolute_error == 0.0
}

fn optional_scalar_final_ok(
    metric: &OptionalScalarErrorMetrics,
    pair_count: u64,
    name: &str,
    bad: &mut Vec<String>,
) {
    match (metric, pair_count) {
        (None, 0) => {}
        (Some(s), n) if n > 0 && scalar_error_exact_zero(s) => {}
        (None, n) if n > 0 => bad.push(format!("{name}: missing while pair_count={n}")),
        (Some(s), 0) => bad.push(format!(
            "{name}: present while pair_count=0 (mae={})",
            s.mae
        )),
        (Some(s), _) => bad.push(format!(
            "{name}: nonzero mae={} rmse={} max={}",
            s.mae, s.rmse, s.maximum_absolute_error
        )),
        (None, _) => {}
    }
}

/// Full scientific + RGB + ray-count exactness for final full-coverage budgets.
pub fn final_scientific_exact(
    is_crop: bool,
    unique_traced_rays: u64,
    sci: &OracleComparisonMetrics,
    rgb: &RgbComparisonMetrics,
    parity: &SampleParityReport,
) -> Result<(), String> {
    let mut bad = Vec::new();
    let expected_rays = final_coverage_ray_count(is_crop);
    if unique_traced_rays != expected_rays {
        bad.push(format!(
            "unique_traced_rays={unique_traced_rays} expected={expected_rays}"
        ));
    }
    if !rgb.exact_match {
        bad.push(format!("rgb not exact mse={}", rgb.channel_mse));
    }
    if parity.selected_sample_mismatch_count != 0 {
        bad.push(format!(
            "sample_parity mismatches={}",
            parity.selected_sample_mismatch_count
        ));
    }
    if sci.outcome_disagreement_count != 0 || sci.outcome_disagreement_rate != 0.0 {
        bad.push(format!(
            "outcome_disagreement count={} rate={}",
            sci.outcome_disagreement_count, sci.outcome_disagreement_rate
        ));
    }
    let rhs = &sci.rhs_absolute_error;
    if rhs.mae != 0.0 || rhs.rmse != 0.0 || rhs.maximum_absolute_error != 0 {
        bad.push(format!(
            "rhs_absolute_error mae={} rmse={} max={}",
            rhs.mae, rhs.rmse, rhs.maximum_absolute_error
        ));
    }
    if sci.celestial_presence_mismatch_count != 0 {
        bad.push(format!(
            "celestial_presence_mismatch={}",
            sci.celestial_presence_mismatch_count
        ));
    }
    if sci.disk_presence_mismatch_count != 0 {
        bad.push(format!(
            "disk_presence_mismatch={}",
            sci.disk_presence_mismatch_count
        ));
    }
    optional_scalar_final_ok(
        &sci.celestial_angular_error_radians,
        sci.celestial_pair_count,
        "celestial_angular",
        &mut bad,
    );
    optional_scalar_final_ok(
        &sci.celestial_wrap_u_error,
        sci.celestial_pair_count,
        "celestial_wrap_u",
        &mut bad,
    );
    optional_scalar_final_ok(
        &sci.celestial_v_error,
        sci.celestial_pair_count,
        "celestial_v",
        &mut bad,
    );
    optional_scalar_final_ok(&sci.log2_g_error, sci.disk_pair_count, "log2_g", &mut bad);
    optional_scalar_final_ok(
        &sci.log2_emitted_error,
        sci.disk_pair_count,
        "log2_emitted",
        &mut bad,
    );
    optional_scalar_final_ok(
        &sci.log2_observed_error,
        sci.disk_pair_count,
        "log2_observed",
        &mut bad,
    );
    if bad.is_empty() {
        Ok(())
    } else {
        Err(bad.join("; "))
    }
}

pub fn encode_outcome_disagreement_pgm(
    oracle: &OracleFrame,
    recon: &AdaptiveReconstruction,
) -> Vec<u8> {
    let mut vals = Vec::with_capacity(oracle.pixels.len());
    for (o, c) in oracle.pixels.iter().zip(&recon.pixels) {
        vals.push(if o.outcome_class == c.outcome_class {
            0
        } else {
            255
        });
    }
    let mut out = format!("P5\n{} {}\n255\n", oracle.width, oracle.height).into_bytes();
    out.extend_from_slice(&vals);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_oracle::{IntegerErrorMetrics, ScalarErrorMetrics};
    use relativity_trace::OutcomeClass;

    fn zero_sci() -> OracleComparisonMetrics {
        OracleComparisonMetrics {
            compared_pixels: 1,
            outcome_disagreement_count: 0,
            outcome_disagreement_rate: 0.0,
            rhs_absolute_error: IntegerErrorMetrics {
                mae: 0.0,
                rmse: 0.0,
                maximum_absolute_error: 0,
                maximum_error_index: 0,
            },
            celestial_pair_count: 0,
            celestial_presence_mismatch_count: 0,
            celestial_angular_error_radians: None,
            celestial_wrap_u_error: None,
            celestial_v_error: None,
            disk_pair_count: 0,
            disk_presence_mismatch_count: 0,
            log2_g_error: None,
            log2_emitted_error: None,
            log2_observed_error: None,
        }
    }

    #[test]
    fn rgb_exact_match_uses_psnr_sentinel() {
        let ppm = b"P6\n1 1\n255\n\x01\x02\x03";
        let m = compare_reconstruction_rgb(ppm, ppm).unwrap();
        assert!(m.exact_match);
        assert_eq!(m.psnr_db, None);
    }

    #[test]
    fn selected_sample_parity_catches_altered_payload() {
        let s = AdaptiveRaySample {
            local_col: 0,
            local_row: 0,
            source_index: 0,
            source_col: 0,
            source_row: 0,
            outcome_class: OutcomeClass::Escaped,
            rhs_evaluations: 1,
            celestial: None,
            disk: None,
            rgb: [0, 0, 0],
        };
        let mut o = OraclePixel {
            local_index: 0,
            col: 0,
            row: 0,
            source_index: 0,
            source_col: 0,
            source_row: 0,
            sensor_x: 0.0,
            sensor_y: 0.0,
            outcome_class: OutcomeClass::Escaped,
            rhs_evaluations: 2,
            failure_class: None,
            celestial: None,
            disk: None,
        };
        assert!(!sample_matches_oracle(&s, &o));
        o.rhs_evaluations = 1;
        assert!(sample_matches_oracle(&s, &o));
    }

    #[test]
    fn final_scientific_exact_accepts_crop_full_coverage() {
        let sci = zero_sci();
        let rgb = RgbComparisonMetrics {
            pixel_count: 1,
            channel_mse: 0.0,
            maximum_absolute_channel_error: 0,
            exact_match: true,
            psnr_db: None,
        };
        let parity = SampleParityReport {
            selected_sample_count: 1,
            selected_sample_exact_count: 1,
            selected_sample_mismatch_count: 0,
        };
        assert!(final_scientific_exact(true, 4096, &sci, &rgb, &parity).is_ok());
        assert!(final_scientific_exact(false, 16384, &sci, &rgb, &parity).is_ok());
    }

    #[test]
    fn final_scientific_exact_rejects_wrong_ray_count_and_nonzero_rhs() {
        let mut sci = zero_sci();
        let rgb = RgbComparisonMetrics {
            pixel_count: 1,
            channel_mse: 0.0,
            maximum_absolute_channel_error: 0,
            exact_match: true,
            psnr_db: None,
        };
        let parity = SampleParityReport {
            selected_sample_count: 1,
            selected_sample_exact_count: 1,
            selected_sample_mismatch_count: 0,
        };
        assert!(final_scientific_exact(true, 4095, &sci, &rgb, &parity).is_err());
        sci.rhs_absolute_error.mae = 1.0;
        assert!(final_scientific_exact(true, 4096, &sci, &rgb, &parity).is_err());
    }

    #[test]
    fn final_scientific_exact_requires_zero_scalars_when_pairs_present() {
        let mut sci = zero_sci();
        sci.celestial_pair_count = 1;
        sci.celestial_angular_error_radians = Some(ScalarErrorMetrics {
            mae: 0.1,
            rmse: 0.1,
            maximum_absolute_error: 0.1,
            maximum_error_index: 0,
        });
        let rgb = RgbComparisonMetrics {
            pixel_count: 1,
            channel_mse: 0.0,
            maximum_absolute_channel_error: 0,
            exact_match: true,
            psnr_db: None,
        };
        let parity = SampleParityReport {
            selected_sample_count: 0,
            selected_sample_exact_count: 0,
            selected_sample_mismatch_count: 0,
        };
        assert!(final_scientific_exact(false, 16384, &sci, &rgb, &parity).is_err());
        sci.celestial_angular_error_radians = Some(ScalarErrorMetrics {
            mae: 0.0,
            rmse: 0.0,
            maximum_absolute_error: 0.0,
            maximum_error_index: 0,
        });
        sci.celestial_wrap_u_error = Some(ScalarErrorMetrics {
            mae: 0.0,
            rmse: 0.0,
            maximum_absolute_error: 0.0,
            maximum_error_index: 0,
        });
        sci.celestial_v_error = Some(ScalarErrorMetrics {
            mae: 0.0,
            rmse: 0.0,
            maximum_absolute_error: 0.0,
            maximum_error_index: 0,
        });
        assert!(final_scientific_exact(false, 16384, &sci, &rgb, &parity).is_ok());
    }
}
