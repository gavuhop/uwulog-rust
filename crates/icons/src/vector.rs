//! Static vector data and rendering engine for in-app UI icons.
//!
//! Generated at compile-time by `build.rs` from `assets/icons/*.svg`.

use crate::IconName;
use egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke};

/// A compiled vector shape primitive representing an icon path or geometric element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VectorShape {
    /// An open stroked path with round line caps and joins.
    StrokedPath {
        points: &'static [(f32, f32)],
        stroke_width: f32,
    },
    /// A closed stroked loop.
    StrokedClosedPath {
        points: &'static [(f32, f32)],
        stroke_width: f32,
    },
    /// A filled polygon (e.g. triangle, beak, arrow head).
    FilledPolygon {
        points: &'static [(f32, f32)],
        opacity: f32,
    },
    /// A stroked circle.
    StrokedCircle {
        cx: f32,
        cy: f32,
        r: f32,
        stroke_width: f32,
    },
    /// A filled circle.
    FilledCircle {
        cx: f32,
        cy: f32,
        r: f32,
        opacity: f32,
    },
    /// A stroked rectangle with corner radius.
    StrokedRect {
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
        corner_r: f32,
        stroke_width: f32,
    },
    /// A filled rectangle with corner radius.
    FilledRect {
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
        corner_r: f32,
        opacity: f32,
    },
    /// An individual line segment (zero allocation).
    LineSegment {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stroke_width: f32,
    },
}

impl VectorShape {
    /// Paints the shape into the painter within the given target rect.
    pub fn paint(&self, painter: &egui::Painter, rect: Rect, _base_stroke: Stroke, color: Color32) {
        let w = rect.width();
        let h = rect.height();
        if w <= 0.0 || h <= 0.0 {
            return;
        }

        let to_pos =
            |x: f32, y: f32| Pos2::new(rect.min.x + (x / 16.0) * w, rect.min.y + (y / 16.0) * h);

        let calc_stroke = |sw: f32| {
            let actual_w = ((sw / 16.0) * w).clamp(1.0, 3.0);
            Stroke::new(actual_w, color)
        };

        let calc_color = |op: f32| {
            if (op - 1.0).abs() < 0.01 {
                color
            } else {
                color.gamma_multiply(op)
            }
        };

        match self {
            Self::StrokedPath {
                points,
                stroke_width,
            } => {
                if points.len() == 2 {
                    let p1 = to_pos(points[0].0, points[0].1);
                    let p2 = to_pos(points[1].0, points[1].1);
                    painter.line_segment([p1, p2], calc_stroke(*stroke_width));
                } else if points.len() > 2 {
                    let pts: Vec<Pos2> = points.iter().map(|&(x, y)| to_pos(x, y)).collect();
                    painter.add(egui::epaint::PathShape::line(
                        pts,
                        calc_stroke(*stroke_width),
                    ));
                }
            }
            Self::StrokedClosedPath {
                points,
                stroke_width,
            } => {
                if points.len() >= 3 {
                    let pts: Vec<Pos2> = points.iter().map(|&(x, y)| to_pos(x, y)).collect();
                    painter.add(egui::epaint::PathShape::closed_line(
                        pts,
                        calc_stroke(*stroke_width),
                    ));
                }
            }
            Self::FilledPolygon { points, opacity } => {
                if points.len() >= 3 {
                    let pts: Vec<Pos2> = points.iter().map(|&(x, y)| to_pos(x, y)).collect();
                    painter.add(egui::epaint::PathShape::convex_polygon(
                        pts,
                        calc_color(*opacity),
                        Stroke::NONE,
                    ));
                }
            }
            Self::StrokedCircle {
                cx,
                cy,
                r,
                stroke_width,
            } => {
                let center = to_pos(*cx, *cy);
                let radius = (r / 16.0) * w;
                painter.circle_stroke(center, radius, calc_stroke(*stroke_width));
            }
            Self::FilledCircle { cx, cy, r, opacity } => {
                let center = to_pos(*cx, *cy);
                let radius = (r / 16.0) * w;
                painter.circle_filled(center, radius, calc_color(*opacity));
            }
            Self::StrokedRect {
                min_x,
                min_y,
                max_x,
                max_y,
                corner_r,
                stroke_width,
            } => {
                let r = Rect::from_min_max(to_pos(*min_x, *min_y), to_pos(*max_x, *max_y));
                let cr = CornerRadius::same(((corner_r / 16.0) * w).round() as u8);
                painter.rect_stroke(r, cr, calc_stroke(*stroke_width), egui::StrokeKind::Inside);
            }
            Self::FilledRect {
                min_x,
                min_y,
                max_x,
                max_y,
                corner_r,
                opacity,
            } => {
                let r = Rect::from_min_max(to_pos(*min_x, *min_y), to_pos(*max_x, *max_y));
                let cr = CornerRadius::same(((corner_r / 16.0) * w).round() as u8);
                painter.rect_filled(r, cr, calc_color(*opacity));
            }
            Self::LineSegment {
                x1,
                y1,
                x2,
                y2,
                stroke_width,
            } => {
                let p1 = to_pos(*x1, *y1);
                let p2 = to_pos(*x2, *y2);
                painter.line_segment([p1, p2], calc_stroke(*stroke_width));
            }
        }
    }
}

// Include generated compile-time vector shape data
include!(concat!(env!("OUT_DIR"), "/icon_vector_data.rs"));
