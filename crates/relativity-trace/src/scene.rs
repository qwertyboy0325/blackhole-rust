//! Trace scene configuration.

use relativity_core::{CameraParams, KerrParams, PositionBl};
use relativity_integrate::{Dop853Config, EventArmingPolicy, IntegrationError};

use crate::camera::TraceGrid;
use crate::disk::ThinDiskGeometry;

/// Complete Gate 1B2 tracing scene (no radiometry).
#[derive(Debug, Clone)]
pub struct TraceScene {
    pub kerr: KerrParams,
    pub observer: PositionBl,
    pub camera: CameraParams,
    pub disk: ThinDiskGeometry,
    pub escape_radius: f64,
    pub integrator: Dop853Config,
    pub event_arming: EventArmingPolicy,
    pub grid: TraceGrid,
}

impl TraceScene {
    pub fn validate(&self) -> Result<(), IntegrationError> {
        self.disk.validate(&self.kerr)?;
        if !self.escape_radius.is_finite() || self.escape_radius <= 0.0 {
            return Err(IntegrationError::InvalidConfig {
                field: "escape_radius",
            });
        }
        self.event_arming.validate()?;
        if self.event_arming != self.integrator.event_arming {
            return Err(IntegrationError::InvalidConfig {
                field: "event_arming",
            });
        }
        let mut cfg = self.integrator.clone();
        cfg.event_arming = self.event_arming.clone();
        cfg.validate()?;
        if self.grid.width == 0 || self.grid.height == 0 {
            return Err(IntegrationError::InvalidConfig { field: "grid" });
        }
        Ok(())
    }

    /// Geometric disk defaults for M≈1 diagnostic scenes (not ISCO).
    pub fn geometric_disk_m1() -> ThinDiskGeometry {
        ThinDiskGeometry::new(3.0, 20.0)
    }
}
