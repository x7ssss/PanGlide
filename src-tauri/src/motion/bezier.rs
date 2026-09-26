use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Point2D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Cubic Bézier curve defined by 4 control points P0, P1, P2, P3
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CubicBezierCurve {
    pub p0: Point2D,
    pub p1: Point2D,
    pub p2: Point2D,
    pub p3: Point2D,
}

impl CubicBezierCurve {
    pub fn new(p0: Point2D, p1: Point2D, p2: Point2D, p3: Point2D) -> Self {
        Self { p0, p1, p2, p3 }
    }

    /// Evaluate curve at parameter t in [0.0, 1.0]
    pub fn evaluate(&self, t: f32) -> Point2D {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;

        let x = uuu * self.p0.x + 3.0 * uu * t * self.p1.x + 3.0 * u * tt * self.p2.x + ttt * self.p3.x;
        let y = uuu * self.p0.y + 3.0 * uu * t * self.p1.y + 3.0 * u * tt * self.p2.y + ttt * self.p3.y;

        Point2D::new(x, y)
    }

    /// Convert 4 consecutive path points into a smooth Catmull-Rom cubic Bézier segment
    pub fn from_catmull_rom(p_prev: Point2D, p0: Point2D, p1: Point2D, p_next: Point2D, tension: f32) -> Self {
        let alpha = (1.0 - tension) / 6.0;

        let p1_ctrl = Point2D::new(
            p0.x + alpha * (p1.x - p_prev.x),
            p0.y + alpha * (p1.y - p_prev.y),
        );

        let p2_ctrl = Point2D::new(
            p1.x - alpha * (p_next.x - p0.x),
            p1.y - alpha * (p_next.y - p0.y),
        );

        Self::new(p0, p1_ctrl, p2_ctrl, p1)
    }
}

/// Spline interpolator that fits a smooth Bézier path over discrete cursor samples
pub struct BezierSplineFitter;

impl BezierSplineFitter {
    pub fn fit_spline(points: &[Point2D]) -> Vec<CubicBezierCurve> {
        if points.len() < 2 {
            return Vec::new();
        }

        if points.len() == 2 {
            let p0 = points[0];
            let p1 = points[1];
            return vec![CubicBezierCurve::new(
                p0,
                Point2D::new(p0.x + (p1.x - p0.x) / 3.0, p0.y + (p1.y - p0.y) / 3.0),
                Point2D::new(p0.x + 2.0 * (p1.x - p0.x) / 3.0, p0.y + 2.0 * (p1.y - p0.y) / 3.0),
                p1,
            )];
        }

        let mut curves = Vec::new();
        let n = points.len();

        for i in 0..(n - 1) {
            let p_prev = if i > 0 { points[i - 1] } else { points[i] };
            let p0 = points[i];
            let p1 = points[i + 1];
            let p_next = if i + 2 < n { points[i + 2] } else { points[i + 1] };

            curves.push(CubicBezierCurve::from_catmull_rom(p_prev, p0, p1, p_next, 0.0));
        }

        curves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cubic_bezier_endpoints() {
        let p0 = Point2D::new(0.0, 0.0);
        let p1 = Point2D::new(10.0, 50.0);
        let p2 = Point2D::new(90.0, 50.0);
        let p3 = Point2D::new(100.0, 100.0);

        let curve = CubicBezierCurve::new(p0, p1, p2, p3);

        let start = curve.evaluate(0.0);
        assert!((start.x - 0.0).abs() < 1e-4);
        assert!((start.y - 0.0).abs() < 1e-4);

        let end = curve.evaluate(1.0);
        assert!((end.x - 100.0).abs() < 1e-4);
        assert!((end.y - 100.0).abs() < 1e-4);

        let mid = curve.evaluate(0.5);
        assert!(mid.x > 30.0 && mid.x < 70.0);
    }
}
