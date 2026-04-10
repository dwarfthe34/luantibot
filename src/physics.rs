use mt_net::{Deg, Point3, Vector3};
use std::collections::HashSet;

/// Block size multiplier
pub const BS: f32 = 10.0;
/// Gravity constant
pub const GRAVITY: f32 = 9.81 * BS;

/// Physics simulation for a bot
#[derive(Debug, Clone)]
pub struct Physics {
    pub vel:        Vector3<f32>,
    pub on_ground:  bool,
    pub want_jump:  bool,
    pub wish_dir:   Vector3<f32>,
    pub walk_speed: f32,
    pub jump_speed: f32,
}

impl Default for Physics {
    fn default() -> Self {
        Self {
            vel: Vector3::new(0.0, 0.0, 0.0),
            on_ground: false,
            want_jump: false,
            wish_dir: Vector3::new(0.0, 0.0, 0.0),
            walk_speed: 4.0 * BS,
            jump_speed: 6.5 * BS,
        }
    }
}

impl Physics {
    /// Update position with full collision against blocks
    pub fn step(&mut self, pos: Point3<f32>, dt: f32, blocks: &HashSet<Point3<i16>>) -> Point3<f32> {
        // Horizontal velocity
        if self.wish_dir.x != 0.0 || self.wish_dir.z != 0.0 {
            self.vel.x = self.wish_dir.x * self.walk_speed;
            self.vel.z = self.wish_dir.z * self.walk_speed;
        } else {
            self.vel.x = 0.0;
            self.vel.z = 0.0;
        }

        // Jump
        if self.want_jump && self.on_ground {
            self.vel.y = self.jump_speed;
            self.on_ground = false;
        }
        self.want_jump = false;

        // Gravity
        self.vel.y -= GRAVITY * dt;

        // Terminal velocity
        const TERMINAL_VEL: f32 = -180.0 * BS;
        self.vel.y = self.vel.y.max(TERMINAL_VEL);

        let mut next = pos + self.vel * dt;

        // Clamp to prevent i32 overflow in PlayerPos serialization
        let max_coord = (i32::MAX as f32) / (100.0 * BS) - 1.0;
        next.x = next.x.clamp(-max_coord, max_coord);
        next.y = next.y.clamp(-max_coord, max_coord);
        next.z = next.z.clamp(-max_coord, max_coord);

        // --- Safe Block Collision ---
        // Calculate bounds in f32 first
        let min_x_f = next.x.floor() - 1.0;
        let max_x_f = next.x.ceil() + 1.0;
        let min_y_f = next.y.floor() - 1.0;
        let max_y_f = next.y.ceil() + 1.0;
        let min_z_f = next.z.floor() - 1.0;
        let max_z_f = next.z.ceil() + 1.0;

        // CLAMP to i16 range BEFORE casting to prevent overflow on subtraction
        // This ensures we never try to calculate (i16::MIN - 1)
        let min_x = min_x_f.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        let max_x = max_x_f.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        let min_y = min_y_f.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        let max_y = max_y_f.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        let min_z = min_z_f.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        let max_z = max_z_f.clamp(i16::MIN as f32, i16::MAX as f32) as i16;

        let mut _collided = false;
        
        for bx in min_x..=max_x {
            for by in min_y..=max_y {
                for bz in min_z..=max_z {
                    let block = Point3::new(bx, by, bz);
                    if blocks.contains(&block) {
                        if next.y >= by as f32 && pos.y < by as f32 {
                            next.y = by as f32;
                            self.vel.y = 0.0;
                            self.on_ground = true;
                            _collided = true;
                        }
                    }
                }
            }
        }

        next
    }

    /// Set movement keys based on yaw and WASD input
    pub fn set_move_keys(
        &mut self,
        yaw: Deg<f32>,
        forward: bool,
        back: bool,
        left: bool,
        right: bool,
    ) {
        let mut dx = 0.0;
        let mut dz = 0.0;

        if forward { dz -= 1.0; }
        if back    { dz += 1.0; }
        if left    { dx -= 1.0; }
        if right   { dx += 1.0; }

        if dx == 0.0 && dz == 0.0 {
            self.wish_dir = Vector3::new(0.0, 0.0, 0.0);
            return;
        }

        let rad = yaw.0.to_radians();
        let sin_y = rad.sin();
        let cos_y = rad.cos();

        let wx = dx * cos_y - dz * sin_y;
        let wz = dx * sin_y + dz * cos_y;
        let len = (wx*wx + wz*wz).sqrt();
        self.wish_dir = Vector3::new(wx / len, 0.0, wz / len);
    }

    /// Update walk/jump speeds
    pub fn apply_movement_params(&mut self, walk_speed: f32, jump_speed: f32) {
        self.walk_speed = walk_speed * BS;
        self.jump_speed = jump_speed * BS;
    }
}