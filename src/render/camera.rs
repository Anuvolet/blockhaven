//! First-person camera and view frustum.

use glam::{Mat4, Vec3, Vec4};

#[derive(Clone, Debug)]
pub struct Camera {
    pub pos: Vec3,
    /// Radians. yaw 0 looks toward -Z; positive yaw turns left.
    pub yaw: f32,
    pub pitch: f32,
    pub fov_deg: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new() -> Camera {
        Camera { pos: Vec3::new(0.0, 80.0, 0.0), yaw: 0.0, pitch: 0.0, fov_deg: 70.0, aspect: 16.0 / 9.0, near: 0.05, far: 1000.0 }
    }
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }
    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(cy, 0.0, -sy)
    }
    pub fn view(&self) -> Mat4 {
        Mat4::look_to_rh(self.pos, self.forward(), Vec3::Y)
    }
    pub fn proj(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_deg.to_radians(), self.aspect, self.near, self.far)
    }
    pub fn view_proj(&self) -> Mat4 {
        self.proj() * self.view()
    }
}

impl Default for Camera {
    fn default() -> Self {
        Camera::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    pub fn from_matrix(m: Mat4) -> Frustum {
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        let mut planes = [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2];
        for p in planes.iter_mut() {
            let n = Vec3::new(p.x, p.y, p.z).length();
            if n > 0.0 {
                *p /= n;
            }
        }
        Frustum { planes }
    }

    /// True if the AABB intersects or is inside the frustum.
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for p in &self.planes {
            let px = if p.x >= 0.0 { max.x } else { min.x };
            let py = if p.y >= 0.0 { max.y } else { min.y };
            let pz = if p.z >= 0.0 { max.z } else { min.z };
            if p.x * px + p.y * py + p.z * pz + p.w < 0.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frustum_culls_behind_camera() {
        let mut cam = Camera::new();
        cam.pos = Vec3::ZERO;
        cam.aspect = 1.0;
        let f = Frustum::from_matrix(cam.view_proj());
        assert!(f.intersects_aabb(Vec3::new(-1.0, -1.0, -20.0), Vec3::new(1.0, 1.0, -18.0)));
        assert!(!f.intersects_aabb(Vec3::new(-1.0, -1.0, 18.0), Vec3::new(1.0, 1.0, 20.0)));
        assert!(!f.intersects_aabb(Vec3::new(100.0, -1.0, -20.0), Vec3::new(101.0, 1.0, -18.0)));
    }
}
