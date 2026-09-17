//! Automated test suite for the UI Icon system.
//!
//! Inspired by Zed's `crates/icons/src/icons.rs` test suite:
//! - Guarantees every `IconName` variant has a corresponding `.svg` asset.
//! - Guarantees no dangling/unregistered `.svg` assets exist on disk.

use super::component::IconSize;
use super::names::IconName;
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn test_all_icon_variants_have_valid_svg_assets() {
    for icon in IconName::ALL {
        let stem = icon.file_stem();
        assert!(
            !stem.is_empty(),
            "Icon {icon:?} must have a valid file stem"
        );

        let content = icon.svg_content();
        assert!(
            content.starts_with("<svg"),
            "SVG content for {stem}.svg must start with '<svg'"
        );
        assert!(
            content.contains("</svg>"),
            "SVG content for {stem}.svg must contain '</svg>'"
        );
    }
}

#[test]
fn test_all_disk_svgs_are_registered_and_no_dangling() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let icons_dir = manifest_dir.join("assets").join("icons");

    assert!(
        icons_dir.exists(),
        "Icons asset directory {:?} must exist",
        icons_dir
    );

    let registered_stems: HashSet<&'static str> =
        IconName::ALL.iter().map(|i| i.file_stem()).collect();

    let mut disk_stems = HashSet::new();

    for entry in std::fs::read_dir(&icons_dir).expect("Failed to read icons directory") {
        let entry = entry.expect("Valid directory entry");
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "svg") {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("Valid UTF-8 file stem")
                .to_string();

            assert!(
                registered_stems.contains(stem.as_str()),
                "Found dangling/unregistered SVG on disk: {stem}.svg. Add it to IconName or remove it."
            );

            disk_stems.insert(stem);
        }
    }

    for registered in &registered_stems {
        assert!(
            disk_stems.contains(*registered),
            "IconName variant '{registered}' does not have a corresponding file on disk in {:?}",
            icons_dir
        );
    }

    assert_eq!(
        registered_stems.len(),
        disk_stems.len(),
        "Count of registered IconName variants must exactly match disk SVGs"
    );
}

#[test]
fn test_icon_sizes() {
    assert_eq!(IconSize::Indicator.px(), 10.0);
    assert_eq!(IconSize::XSmall.px(), 12.0);
    assert_eq!(IconSize::Small.px(), 14.0);
    assert_eq!(IconSize::Medium.px(), 16.0);
    assert_eq!(IconSize::Large.px(), 20.0);
    assert_eq!(IconSize::XLarge.px(), 24.0);
    assert_eq!(IconSize::Custom(18.5).px(), 18.5);
}

#[test]
fn test_all_icon_variants_have_compiled_vector_shapes() {
    for icon in IconName::ALL {
        let shapes = icon.vector_shapes();
        assert!(
            !shapes.is_empty(),
            "Icon {icon:?} must have at least one compiled VectorShape"
        );

        for (idx, shape) in shapes.iter().enumerate() {
            match shape {
                crate::components::ui::icon::VectorShape::StrokedPath {
                    points,
                    stroke_width,
                }
                | crate::components::ui::icon::VectorShape::StrokedClosedPath {
                    points,
                    stroke_width,
                } => {
                    assert!(
                        points.len() >= 2,
                        "Icon {icon:?} shape #{idx} path must have >= 2 points"
                    );
                    assert!(stroke_width.is_finite() && *stroke_width > 0.0);
                    for pt in *points {
                        assert!(
                            pt.0.is_finite() && pt.1.is_finite(),
                            "Icon {icon:?} shape #{idx} point {pt:?} must be finite"
                        );
                    }
                }
                crate::components::ui::icon::VectorShape::FilledPolygon { points, opacity } => {
                    assert!(
                        points.len() >= 3,
                        "Icon {icon:?} shape #{idx} polygon must have >= 3 points"
                    );
                    assert!(opacity.is_finite() && *opacity > 0.0);
                    for pt in *points {
                        assert!(
                            pt.0.is_finite() && pt.1.is_finite(),
                            "Icon {icon:?} shape #{idx} point {pt:?} must be finite"
                        );
                    }
                }
                crate::components::ui::icon::VectorShape::StrokedCircle {
                    cx,
                    cy,
                    r,
                    stroke_width,
                } => {
                    assert!(cx.is_finite() && cy.is_finite() && r.is_finite() && *r > 0.0);
                    assert!(stroke_width.is_finite() && *stroke_width > 0.0);
                }
                crate::components::ui::icon::VectorShape::FilledCircle { cx, cy, r, opacity } => {
                    assert!(cx.is_finite() && cy.is_finite() && r.is_finite() && *r > 0.0);
                    assert!(opacity.is_finite() && *opacity > 0.0);
                }
                crate::components::ui::icon::VectorShape::StrokedRect {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    corner_r,
                    stroke_width,
                } => {
                    assert!(
                        min_x.is_finite()
                            && min_y.is_finite()
                            && max_x.is_finite()
                            && max_y.is_finite()
                            && corner_r.is_finite()
                    );
                    assert!(stroke_width.is_finite() && *stroke_width > 0.0);
                }
                crate::components::ui::icon::VectorShape::FilledRect {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    corner_r,
                    opacity,
                } => {
                    assert!(
                        min_x.is_finite()
                            && min_y.is_finite()
                            && max_x.is_finite()
                            && max_y.is_finite()
                            && corner_r.is_finite()
                    );
                    assert!(opacity.is_finite() && *opacity > 0.0);
                }
                crate::components::ui::icon::VectorShape::LineSegment {
                    x1,
                    y1,
                    x2,
                    y2,
                    stroke_width,
                } => {
                    assert!(x1.is_finite() && y1.is_finite() && x2.is_finite() && y2.is_finite());
                    assert!(stroke_width.is_finite() && *stroke_width > 0.0);
                }
            }
        }
    }
}
