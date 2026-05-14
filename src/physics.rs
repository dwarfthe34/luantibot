use mt_net::{Deg, Point3, Vector3};
use std::collections::HashSet;

pub const BS: f32 = 10.0;
pub const GRAVITY: f32 = 9.81 * BS;

#[derive(Debug, Clone)]
pub struct Physics {
    pub vel:        Vector3<f32>,
    pub on_ground:  bool,
    pub want_jump:  bool,
    pub wish_dir:   Vector3<f32>,
    pub walk_speed: f32,
    pub jump_speed: f32,
    pub gravity:    f32,
}

impl Default for Physics {
    fn default() -> Self {
        Self {
            vel:        Vector3::new(0.0, 0.0, 0.0),
            on_ground:  false,
            want_jump:  false,
            wish_dir:   Vector3::new(0.0, 0.0, 0.0),
            walk_speed: 4.0 * BS,
            jump_speed: 6.5 * BS,
            gravity:    GRAVITY,
        }
    }
}

impl Physics {
    pub fn step(
        &mut self,
        pos: Point3<f32>,
        dt: f32,
        blocks: &HashSet<Point3<i16>>,
    ) -> Point3<f32> {
        // Horizontal movement
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

        // Apply gravity only when not on ground
        if !self.on_ground {
            self.vel.y -= self.gravity * dt;
        }

        // Terminal velocity
        let terminal = -180.0 * BS;
        if self.vel.y < terminal {
            self.vel.y = terminal;
        }

        let mut next = pos + self.vel * dt;

        // Clamp world bounds
        let max_coord = (i32::MAX as f32) / (100.0 * BS) - 1.0;
        next.x = next.x.clamp(-max_coord, max_coord);
        next.y = next.y.clamp(-max_coord, max_coord);
        next.z = next.z.clamp(-max_coord, max_coord);

        // Horizontal AABB extents
        let half_size = 0.3 * BS;
        let min_x = ((next.x - half_size) / BS).floor() as i32;
        let max_x = ((next.x + half_size) / BS).floor() as i32;
        let min_z = ((next.z - half_size) / BS).floor() as i32;
        let max_z = ((next.z + half_size) / BS).floor() as i32;

        // --- Vertical collision (falling) ---
        // In Minetest, node at integer block_y has its center at block_y*BS,
        // so its top surface is at (block_y + 0.5) * BS.
        //
        // We sweep from the lowest block the player could reach (next.y)
        // up to where they started (pos.y), checking each block layer.
        // This prevents tunneling at high fall speeds.
        if self.vel.y <= 0.0 {
            let check_from = (next.y / BS).floor() as i32;
            let check_to   = (pos.y  / BS).floor() as i32;

            // Iterate from highest to lowest so we land on the topmost block
            'outer: for by in (check_from..=check_to).rev() {
                let top = (by as f32 + 0.5) * BS;

                // Only snap if feet are at or below this block's top surface
                if next.y > top {
                    continue;
                }

                for bx in min_x..=max_x {
                    for bz in min_z..=max_z {
                        let key = Point3::new(bx as i16, by as i16, bz as i16);
                        if blocks.contains(&key) {
                            next.y = top;
                            self.vel.y = 0.0;
                            self.on_ground = true;
                            break 'outer;
                        }
                    }
                }
            }

            // If no block found below, we're in the air
            if self.vel.y < 0.0 {
                self.on_ground = false;
            }
        }

        // --- Vertical collision (rising) ---
        // Prevent clipping into a ceiling when jumping
        if self.vel.y > 0.0 {
            // Player is ~1.75 blocks tall; check head position
            let head_y = next.y + 1.75 * BS;
            let by = (head_y / BS).floor() as i32;
            let ceil_bottom = (by as f32 - 0.5) * BS; // bottom of block above

            if head_y >= ceil_bottom {
                for bx in min_x..=max_x {
                    for bz in min_z..=max_z {
                        let key = Point3::new(bx as i16, by as i16, bz as i16);
                        if blocks.contains(&key) {
                            self.vel.y = 0.0;
                            next.y = ceil_bottom - 1.75 * BS;
                            break;
                        }
                    }
                }
            }
        }

        next
    }

    pub fn set_move_keys(
        &mut self,
        yaw: Deg<f32>,
        forward: bool,
        back: bool,
        left: bool,
        right: bool,
    ) {
        let mut dx = 0.0f32;
        let mut dz = 0.0f32;

        if forward { dz -= 1.0; }
        if back    { dz += 1.0; }
        if left    { dx -= 1.0; }
        if right   { dx += 1.0; }

        if dx == 0.0 && dz == 0.0 {
            self.wish_dir = Vector3::new(0.0, 0.0, 0.0);
            return;
        }

        let rad = yaw.0.to_radians();
        let wx = dx * rad.cos() - dz * rad.sin();
        let wz = dx * rad.sin() + dz * rad.cos();
        let len = (wx * wx + wz * wz).sqrt();

        self.wish_dir = Vector3::new(wx / len, 0.0, wz / len);
    }

    pub fn apply_movement_params(&mut self, walk_speed: f32, jump_speed: f32, gravity: f32) {
        self.walk_speed = walk_speed * BS;
        self.jump_speed = jump_speed * BS;
        self.gravity    = gravity * BS;
    }
}
