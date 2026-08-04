//! Bounded Kerr camera-ray corpus for Gate 1B2.

use relativity_core::{CameraParams, KerrParams, PositionBl, SensorCoord};
use relativity_integrate::{
    Dop853Config, EventArmingPolicy, HorizonProximityPolicy, IntegrationError,
};

use crate::camera::TraceGrid;
use crate::disk::ThinDiskGeometry;
use crate::outcome::{OutcomeClass, RayOutcome};
use crate::scene::TraceScene;
use crate::trace::trace_ray_sensor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusId {
    Center,
    LeftEquatorial,
    RightEquatorial,
    AboveDisk,
    BelowDisk,
    NearAxis,
    ProgradeSide,
    RetrogradeSide,
    HighSpinExterior,
    ExpectEscape,
    ExpectDisk,
    ExpectHorizonApproach,
}

impl CorpusId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::LeftEquatorial => "left_equatorial",
            Self::RightEquatorial => "right_equatorial",
            Self::AboveDisk => "above_disk",
            Self::BelowDisk => "below_disk",
            Self::NearAxis => "near_axis",
            Self::ProgradeSide => "prograde_side",
            Self::RetrogradeSide => "retrograde_side",
            Self::HighSpinExterior => "high_spin_exterior",
            Self::ExpectEscape => "expect_escape",
            Self::ExpectDisk => "expect_disk",
            Self::ExpectHorizonApproach => "expect_horizon_approach",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraCorpusCase {
    pub id: CorpusId,
    pub scene: TraceScene,
    pub sensor: SensorCoord,
    pub expected: OutcomeClass,
}

fn scene_at(
    spin: f64,
    r_obs: f64,
    theta_deg: f64,
    fov_deg: f64,
    escape: f64,
) -> Result<TraceScene, IntegrationError> {
    let kerr = KerrParams::new(1.0, spin).map_err(IntegrationError::from_core)?;
    let r_plus = kerr.outer_horizon_radius();
    // Geometric annulus — not ISCO.
    let disk = ThinDiskGeometry::new((r_plus + 1.5).max(3.0), 20.0);
    disk.validate(&kerr)?;
    let mut integrator = Dop853Config::diagnostic_default();
    integrator.affine_limit = 400.0;
    integrator.max_step = 0.5;
    integrator.horizon_proximity = HorizonProximityPolicy::enabled(1e-4)?;
    integrator.event_arming = EventArmingPolicy::after(1e-12)?;
    Ok(TraceScene {
        kerr,
        observer: PositionBl::new(0.0, r_obs, theta_deg.to_radians(), 0.0),
        camera: CameraParams {
            horizontal_fov: fov_deg.to_radians(),
            roll: 0.0,
        },
        disk,
        escape_radius: escape,
        event_arming: integrator.event_arming.clone(),
        integrator,
        grid: TraceGrid {
            width: 1,
            height: 1,
        },
    })
}

pub fn camera_corpus() -> Result<Vec<CameraCorpusCase>, IntegrationError> {
    // Expectations calibrated from declared sensors (not tuned post-hoc beyond probe).
    Ok(vec![
        CameraCorpusCase {
            id: CorpusId::Center,
            scene: scene_at(0.5, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.0, y: 0.0 },
            // Center look toward BH: exact horizon root observed for this fixture.
            expected: OutcomeClass::HorizonEvent,
        },
        CameraCorpusCase {
            id: CorpusId::LeftEquatorial,
            scene: scene_at(0.5, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: -0.35, y: 0.0 },
            expected: OutcomeClass::HorizonEvent,
        },
        CameraCorpusCase {
            id: CorpusId::RightEquatorial,
            scene: scene_at(0.5, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.35, y: 0.0 },
            expected: OutcomeClass::HorizonEvent,
        },
        CameraCorpusCase {
            id: CorpusId::AboveDisk,
            scene: scene_at(0.5, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.0, y: 0.55 },
            expected: OutcomeClass::DiskHit,
        },
        CameraCorpusCase {
            id: CorpusId::BelowDisk,
            scene: scene_at(0.5, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.0, y: -0.55 },
            expected: OutcomeClass::DiskHit,
        },
        CameraCorpusCase {
            id: CorpusId::NearAxis,
            scene: scene_at(0.5, 20.0, 20.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.0, y: 0.0 },
            expected: OutcomeClass::HorizonEvent,
        },
        CameraCorpusCase {
            id: CorpusId::ProgradeSide,
            scene: scene_at(0.9, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.25, y: 0.0 },
            expected: OutcomeClass::HorizonApproach,
        },
        CameraCorpusCase {
            id: CorpusId::RetrogradeSide,
            scene: scene_at(0.9, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: -0.25, y: 0.0 },
            expected: OutcomeClass::HorizonApproach,
        },
        CameraCorpusCase {
            id: CorpusId::HighSpinExterior,
            scene: scene_at(0.999, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.6, y: 0.2 },
            expected: OutcomeClass::DiskHit,
        },
        CameraCorpusCase {
            id: CorpusId::ExpectEscape,
            scene: scene_at(0.0, 30.0, 85.0, 70.0, 60.0)?,
            sensor: SensorCoord { x: 0.95, y: 0.95 },
            expected: OutcomeClass::Escaped,
        },
        CameraCorpusCase {
            id: CorpusId::ExpectDisk,
            scene: scene_at(0.5, 20.0, 85.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.0, y: 0.4 },
            expected: OutcomeClass::DiskHit,
        },
        CameraCorpusCase {
            id: CorpusId::ExpectHorizonApproach,
            scene: scene_at(0.0, 20.0, 90.0, 50.0, 80.0)?,
            sensor: SensorCoord { x: 0.0, y: 0.0 },
            expected: OutcomeClass::HorizonApproach,
        },
    ])
}

pub fn run_camera_corpus() -> Result<Vec<(CorpusId, RayOutcome)>, IntegrationError> {
    let cases = camera_corpus()?;
    let mut out = Vec::with_capacity(cases.len());
    for case in &cases {
        let outcome = trace_ray_sensor(&case.scene, case.sensor)?;
        if outcome.class() != case.expected {
            return Err(IntegrationError::EventDomain {
                event_id: relativity_integrate::EventId::ThinDisk,
                detail: format!(
                    "{}: expected {:?}, got {:?}",
                    case.id.as_str(),
                    case.expected,
                    outcome.class()
                ),
            });
        }
        out.push((case.id, outcome));
    }
    Ok(out)
}
