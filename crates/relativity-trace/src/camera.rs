//! Camera-grid sensor mapping (pixel centers).

use relativity_core::SensorCoord;

/// Image grid dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TraceGrid {
    pub width: u32,
    pub height: u32,
}

impl TraceGrid {
    pub fn pixel_count(&self) -> usize {
        (self.width as usize).saturating_mul(self.height as usize)
    }
}

/// Deterministic pixel-center → sensor mapping.
///
/// Convention (fixed; not tuned for visual preference):
/// - sensor domain: `[-1, 1]²` matching [`SensorCoord`]
/// - column `i` increases left → right: `x = 2*(i+0.5)/width - 1`
/// - row `j` increases top → bottom: `y = 1 - 2*(j+0.5)/height`
/// - row-major linear index: `j * width + i`
/// - camera handedness: as defined by `initialize_rectilinear_ray` (dir_x, dir_y)
pub fn sensor_at_pixel_center(grid: TraceGrid, col: u32, row: u32) -> SensorCoord {
    debug_assert!(col < grid.width && row < grid.height);
    let x = 2.0 * (f64::from(col) + 0.5) / f64::from(grid.width) - 1.0;
    let y = 1.0 - 2.0 * (f64::from(row) + 0.5) / f64::from(grid.height);
    SensorCoord { x, y }
}

pub fn pixel_index(grid: TraceGrid, col: u32, row: u32) -> usize {
    (row as usize) * (grid.width as usize) + (col as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_pixel_near_origin_for_odd_grid() {
        let g = TraceGrid {
            width: 3,
            height: 3,
        };
        let s = sensor_at_pixel_center(g, 1, 1);
        assert!((s.x).abs() < 1e-15);
        assert!((s.y).abs() < 1e-15);
    }

    #[test]
    fn corners_have_expected_signs() {
        let g = TraceGrid {
            width: 4,
            height: 4,
        };
        let tl = sensor_at_pixel_center(g, 0, 0);
        assert!(tl.x < 0.0 && tl.y > 0.0);
        let br = sensor_at_pixel_center(g, 3, 3);
        assert!(br.x > 0.0 && br.y < 0.0);
    }

    #[test]
    fn row_major_index() {
        let g = TraceGrid {
            width: 5,
            height: 3,
        };
        assert_eq!(pixel_index(g, 4, 2), 14);
    }
}
