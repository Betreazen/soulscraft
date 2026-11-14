//! Small vector helpers in Minecraft's world frame: +x east, +y up, +z south; a yaw of 0 faces
//! south (+z) and 90 west (−x), as `Entity.getYRot` counts it.

pub type V3 = [f64; 3];

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn len(a: V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

pub fn dist(a: V3, b: V3) -> f64 {
    len(sub(a, b))
}

/// Distance on the ground plane.
pub fn flat_dist(a: V3, b: V3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The unit vector on the ground from `from` to `to`; south when they stand on each other.
pub fn flat_dir(from: V3, to: V3) -> V3 {
    let d = [to[0] - from[0], 0.0, to[2] - from[2]];
    let l = len(d);
    if l < 1e-6 { [0.0, 0.0, 1.0] } else { scale(d, 1.0 / l) }
}

/// The way a yaw faces, on the ground.
pub fn yaw_dir(yaw_deg: f64) -> V3 {
    let r = yaw_deg.to_radians();
    [-r.sin(), 0.0, r.cos()]
}

/// The yaw that faces along `dir`.
pub fn dir_yaw(dir: V3) -> f64 {
    (-dir[0]).atan2(dir[2]).to_degrees()
}

/// The angle in degrees between where a yaw faces and the way to `to` from `from`, 0 to 180.
pub fn off_facing(from: V3, yaw_deg: f64, to: V3) -> f64 {
    let f = yaw_dir(yaw_deg);
    let d = flat_dir(from, to);
    (f[0] * d[0] + f[2] * d[2]).clamp(-1.0, 1.0).acos().to_degrees()
}

/// The point at `radius` from `centre` on the ground, `angle` radians round from south.
pub fn ring_point(centre: V3, radius: f64, angle: f64) -> V3 {
    [centre[0] + radius * angle.sin(), centre[1], centre[2] + radius * angle.cos()]
}

/// The angle `ring_point` gives `p` round `centre`.
pub fn ring_angle(centre: V3, p: V3) -> f64 {
    (p[0] - centre[0]).atan2(p[2] - centre[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn yaw_follows_minecraft() {
        let s = yaw_dir(0.0);
        assert!(close(s[2], 1.0) && close(s[0], 0.0));
        let w = yaw_dir(90.0);
        assert!(close(w[0], -1.0));
        assert!(close(dir_yaw([-1.0, 0.0, 0.0]), 90.0));
        assert!(close(off_facing([0.0; 3], 0.0, [0.0, 0.0, 5.0]), 0.0));
        assert!(close(off_facing([0.0; 3], 0.0, [0.0, 0.0, -5.0]), 180.0));
        assert!(close(off_facing([0.0; 3], 90.0, [-3.0, 0.0, 0.0]), 0.0));
    }

    #[test]
    fn rings_and_distances() {
        let p = ring_point([1.0, 5.0, 1.0], 2.0, 0.0);
        assert!(close(p[2], 3.0) && close(p[1], 5.0));
        assert!(close(ring_angle([1.0, 5.0, 1.0], p), 0.0));
        assert!(close(dist([0.0; 3], [3.0, 4.0, 0.0]), 5.0));
        assert!(close(flat_dist([0.0; 3], [3.0, 9.0, 4.0]), 5.0));
        assert_eq!(flat_dir([1.0; 3], [1.0; 3]), [0.0, 0.0, 1.0]);
        assert_eq!(add([1.0; 3], scale([1.0, 2.0, 3.0], 2.0)), [3.0, 5.0, 7.0]);
    }
}
