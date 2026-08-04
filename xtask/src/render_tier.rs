//! Named diagnostic-render tiers (Gate 2A0-4).
//!
//! Tiers differ only in image dimensions and authority classification.
//! They do not change numerical or physical tracing semantics.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Hard safety bound on a single grid axis. Validated before allocation.
pub const MAX_DIAGNOSTIC_DIMENSION: u32 = 4096;

/// Default axis used by legacy CLI and partial custom overrides.
pub const LEGACY_DEFAULT_AXIS: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticRenderTier {
    Smoke,
    Preview,
    Gate,
    Showcase,
}

impl DiagnosticRenderTier {
    pub const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Smoke => (32, 32),
            Self::Preview => (64, 64),
            Self::Gate => (128, 128),
            Self::Showcase => (256, 256),
        }
    }

    pub const fn authority_class(self) -> RenderAuthorityClass {
        match self {
            Self::Gate => RenderAuthorityClass::AuthoritativeCandidate,
            Self::Smoke | Self::Preview | Self::Showcase => RenderAuthorityClass::NonAuthoritative,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::Preview => "preview",
            Self::Gate => "gate",
            Self::Showcase => "showcase",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolutionSource {
    NamedTier,
    CustomDimensions,
    LegacyDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderAuthorityClass {
    AuthoritativeCandidate,
    NonAuthoritative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRenderPlan {
    pub tier: Option<DiagnosticRenderTier>,
    pub width: u32,
    pub height: u32,
    pub resolution_source: ResolutionSource,
    pub authority_class: RenderAuthorityClass,
}

/// Resolve CLI tier/dimension arguments into a validated render plan.
///
/// Does not allocate a pixel buffer — only validates dimensions.
pub fn resolve_render_plan(
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<ResolvedRenderPlan, String> {
    if let Some(t) = tier {
        if width.is_some() || height.is_some() {
            return Err(format!(
                "named tier `{tier}` rejects explicit --width/--height; omit dimensions or omit --tier",
                tier = t.as_str()
            ));
        }
        let (w, h) = t.dimensions();
        validate_dimensions(w, h)?;
        return Ok(ResolvedRenderPlan {
            tier: Some(t),
            width: w,
            height: h,
            resolution_source: ResolutionSource::NamedTier,
            authority_class: t.authority_class(),
        });
    }

    if width.is_none() && height.is_none() {
        let (w, h) = (LEGACY_DEFAULT_AXIS, LEGACY_DEFAULT_AXIS);
        validate_dimensions(w, h)?;
        return Ok(ResolvedRenderPlan {
            tier: None,
            width: w,
            height: h,
            resolution_source: ResolutionSource::LegacyDefault,
            authority_class: RenderAuthorityClass::NonAuthoritative,
        });
    }

    let w = width.unwrap_or(LEGACY_DEFAULT_AXIS);
    let h = height.unwrap_or(LEGACY_DEFAULT_AXIS);
    validate_dimensions(w, h)?;
    Ok(ResolvedRenderPlan {
        tier: None,
        width: w,
        height: h,
        resolution_source: ResolutionSource::CustomDimensions,
        authority_class: RenderAuthorityClass::NonAuthoritative,
    })
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("dimensions must be positive (zero rejected)".into());
    }
    if width > MAX_DIAGNOSTIC_DIMENSION || height > MAX_DIAGNOSTIC_DIMENSION {
        return Err(format!(
            "dimensions {width}×{height} exceed safety limit {MAX_DIAGNOSTIC_DIMENSION}×{MAX_DIAGNOSTIC_DIMENSION}"
        ));
    }
    let _pixel_count = (width as u64)
        .checked_mul(height as u64)
        .ok_or_else(|| format!("dimensions {width}×{height} overflow u64 pixel count"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_tiers_resolve_exact_dimensions() {
        assert_eq!(DiagnosticRenderTier::Smoke.dimensions(), (32, 32));
        assert_eq!(DiagnosticRenderTier::Preview.dimensions(), (64, 64));
        assert_eq!(DiagnosticRenderTier::Gate.dimensions(), (128, 128));
        assert_eq!(DiagnosticRenderTier::Showcase.dimensions(), (256, 256));
    }

    #[test]
    fn gate_is_only_authoritative_candidate() {
        assert_eq!(
            DiagnosticRenderTier::Gate.authority_class(),
            RenderAuthorityClass::AuthoritativeCandidate
        );
        for t in [
            DiagnosticRenderTier::Smoke,
            DiagnosticRenderTier::Preview,
            DiagnosticRenderTier::Showcase,
        ] {
            assert_eq!(
                t.authority_class(),
                RenderAuthorityClass::NonAuthoritative,
                "{t:?}"
            );
        }
    }

    #[test]
    fn named_tier_plus_width_rejected() {
        let err =
            resolve_render_plan(Some(DiagnosticRenderTier::Preview), Some(64), None).unwrap_err();
        assert!(err.contains("rejects"), "{err}");
    }

    #[test]
    fn named_tier_plus_height_rejected() {
        let err =
            resolve_render_plan(Some(DiagnosticRenderTier::Gate), None, Some(128)).unwrap_err();
        assert!(err.contains("rejects"), "{err}");
    }

    #[test]
    fn custom_width_only_preserves_default_height() {
        let p = resolve_render_plan(None, Some(96), None).unwrap();
        assert_eq!((p.width, p.height), (96, 128));
        assert_eq!(p.resolution_source, ResolutionSource::CustomDimensions);
        assert_eq!(p.authority_class, RenderAuthorityClass::NonAuthoritative);
        assert!(p.tier.is_none());
    }

    #[test]
    fn custom_height_only_preserves_default_width() {
        let p = resolve_render_plan(None, None, Some(72)).unwrap();
        assert_eq!((p.width, p.height), (128, 72));
        assert_eq!(p.resolution_source, ResolutionSource::CustomDimensions);
    }

    #[test]
    fn no_arguments_legacy_128() {
        let p = resolve_render_plan(None, None, None).unwrap();
        assert_eq!((p.width, p.height), (128, 128));
        assert_eq!(p.resolution_source, ResolutionSource::LegacyDefault);
        assert_eq!(p.authority_class, RenderAuthorityClass::NonAuthoritative);
        assert!(p.tier.is_none());
    }

    #[test]
    fn zero_dimensions_rejected() {
        assert!(resolve_render_plan(None, Some(0), Some(128)).is_err());
        assert!(resolve_render_plan(None, Some(128), Some(0)).is_err());
    }

    #[test]
    fn safety_limit_rejected_before_allocation() {
        let err =
            resolve_render_plan(None, Some(MAX_DIAGNOSTIC_DIMENSION + 1), Some(1)).unwrap_err();
        assert!(err.contains("safety limit"), "{err}");
    }

    #[test]
    fn custom_128_not_authoritative() {
        let p = resolve_render_plan(None, Some(128), Some(128)).unwrap();
        assert_eq!(p.authority_class, RenderAuthorityClass::NonAuthoritative);
        assert_eq!(p.resolution_source, ResolutionSource::CustomDimensions);
    }

    #[test]
    fn explicit_gate_is_authoritative_candidate() {
        let p = resolve_render_plan(Some(DiagnosticRenderTier::Gate), None, None).unwrap();
        assert_eq!((p.width, p.height), (128, 128));
        assert_eq!(p.resolution_source, ResolutionSource::NamedTier);
        assert_eq!(
            p.authority_class,
            RenderAuthorityClass::AuthoritativeCandidate
        );
    }
}
