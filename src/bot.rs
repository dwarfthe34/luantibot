use mt_net::{
    enumset::EnumSet, CltSender, Deg, Key, PlayerPos, Point3, Rad, SenderExt, ToSrvPkt, Vector3,
    CONTENT_AIR, CONTENT_IGNORE, CONTENT_UNKNOWN,
};

use std::collections::HashSet;

use crate::{config::Config, error::BotError, event::Event, net, state::BotState};

pub const BS: f32 = 10.0;
pub const GRAVITY: f32 = 9.81 * BS;

#[derive(Debug, Clone)]
pub struct Physics {
    pub vel: Vector3<f32>,
    pub on_ground: bool,
    pub want_jump: bool,
    pub wish_dir: Vector3<f32>,
    pub walk_speed: f32,
    pub jump_speed: f32,
    pub gravity: f32,
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
            gravity: GRAVITY,
        }
    }
}

fn mapblock_coord(node_coord: i32) -> i16 {
    node_coord
        .div_euclid(16)
        .clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn is_collidable_node(id: u16) -> bool {
    // Luanti reserves explicit IDs for air/ignore/unknown. Any other content
    // ID can be a real node (including 0 on some worlds), so keep it solid
    // until node definitions are tracked well enough to inspect collision_box.
    !matches!(id, CONTENT_AIR | CONTENT_IGNORE | CONTENT_UNKNOWN)
}

pub struct Bot {
    tx: CltSender,
    event_rx: tokio::sync::mpsc::Receiver<Event>,
    pub state: BotState,
    username: String,
}

impl Bot {
    pub async fn connect(cfg: Config) -> Result<Self, BotError> {
        let username = cfg.username.clone();
        let handle = net::connect_bot(cfg).await?;

        Ok(Self {
            tx: handle.tx,
            event_rx: handle.event_rx,
            state: BotState::default(),
            username,
        })
    }

    pub async fn connect_str() -> Result<Self, BotError> {
        let address = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "127.0.0.1:30000".into());

        let username = std::env::args().nth(2).unwrap_or_else(|| "bot".into());

        let password = std::env::args().nth(3).unwrap_or_else(|| "password".into());

        Self::connect(Config::new(address, username, password)).await
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub async fn next_event(&mut self) -> Option<Event> {
        let event = self.event_rx.recv().await?;

        match &event {
            Event::Joined => {
                self.state.joined = true;
            }

            Event::MovePlayer { pos, pitch, yaw } => {
                let old_y = self.state.pos.y;

                self.state.pos = *pos;
                self.state.pitch = *pitch;
                self.state.yaw = *yaw;

                if pos.y > old_y + BS {
                    self.state.physics.vel.y = 0.0;
                    self.state.physics.on_ground = false;
                }
            }

            Event::Hp { hp } => {
                self.state.hp = *hp;
            }

            Event::MovementParams {
                walk_speed,
                jump_speed,
                gravity,
            } => {
                self.state
                    .physics
                    .apply_movement_params(*walk_speed, *jump_speed, *gravity);
            }

            Event::BlockData { pos, param0 } => {
                self.state.loaded_mapblocks.insert(*pos);

                let bx = pos.x as i32 * 16;
                let by = pos.y as i32 * 16;
                let bz = pos.z as i32 * 16;

                for (i, &id) in param0.iter().enumerate() {
                    let dx = (i % 16) as i32;
                    let dy = (i / 16 % 16) as i32;
                    let dz = (i / 256) as i32;

                    let nx = (bx + dx).clamp(i16::MIN as i32, i16::MAX as i32) as i16;

                    let ny = (by + dy).clamp(i16::MIN as i32, i16::MAX as i32) as i16;

                    let nz = (bz + dz).clamp(i16::MIN as i32, i16::MAX as i32) as i16;

                    let node_pos = Point3::new(nx, ny, nz);

                    if is_collidable_node(id) {
                        self.state.blocks.insert(node_pos);
                    } else {
                        self.state.blocks.remove(&node_pos);
                    }
                }
            }

            _ => {}
        }

        Some(event)
    }

    pub async fn physics_step(&mut self, dt: f32) -> Result<(), BotError> {
        if !self.has_collision_data_near(self.state.pos) {
            self.state.physics.vel.y = 0.0;
            return self
                .send_pos(
                    self.state.pos,
                    self.state.physics.vel,
                    self.state.pitch,
                    self.state.yaw,
                    EnumSet::empty(),
                )
                .await;
        }

        let new_pos = self
            .state
            .physics
            .step(self.state.pos, dt, &self.state.blocks);

        self.state.pos = new_pos;

        let vel = self.state.physics.vel;
        let pitch = self.state.pitch;
        let yaw = self.state.yaw;

        self.send_pos(new_pos, vel, pitch, yaw, EnumSet::empty())
            .await
    }

    fn has_collision_data_near(&self, pos: Point3<f32>) -> bool {
        let node_x = (pos.x / BS).floor() as i32;
        let node_y = (pos.y / BS).floor() as i32;
        let node_z = (pos.z / BS).floor() as i32;
        let map_x = mapblock_coord(node_x);
        let map_z = mapblock_coord(node_z);

        // Missing map blocks are not the same thing as air. If we simulate
        // gravity before the server has sent the block column around the bot,
        // the empty collision cache makes the bot report positions below the
        // world forever. Only run gravity after we have map data for the bot's
        // current column and the block just below its feet.
        [node_y, node_y - 1]
            .into_iter()
            .map(mapblock_coord)
            .any(|map_y| {
                self.state
                    .loaded_mapblocks
                    .contains(&Point3::new(map_x, map_y, map_z))
            })
    }

    pub fn look(&mut self, yaw: Deg<f32>, pitch: Deg<f32>) {
        self.state.yaw = yaw;
        self.state.pitch = pitch;
    }

    pub fn walk(&mut self, forward: bool, back: bool, left: bool, right: bool) {
        let yaw = self.state.yaw;

        self.state
            .physics
            .set_move_keys(yaw, forward, back, left, right);
    }

    pub fn stop(&mut self) {
        self.state.physics.wish_dir = Vector3::new(0.0, 0.0, 0.0);
    }

    pub fn jump(&mut self) {
        self.state.physics.want_jump = true;
    }

    pub async fn send_chat(&self, msg: impl Into<String>) -> Result<(), BotError> {
        self.tx
            .send(&ToSrvPkt::ChatMsg { msg: msg.into() })
            .await
            .map(|_| ())
            .map_err(|e| BotError::Net(e.to_string()))
    }

    pub async fn send_pos(
        &self,
        pos: Point3<f32>,
        vel: Vector3<f32>,
        pitch: Deg<f32>,
        yaw: Deg<f32>,
        keys: EnumSet<Key>,
    ) -> Result<(), BotError> {
        self.tx
            .send(&ToSrvPkt::PlayerPos(PlayerPos {
                pos,
                vel,
                pitch,
                yaw,
                keys,
                fov: Rad(std::f32::consts::FRAC_PI_2),
                wanted_range: 12,
            }))
            .await
            .map(|_| ())
            .map_err(|e| BotError::Net(e.to_string()))
    }

    pub async fn send_pos_simple(&self, pos: Point3<f32>, yaw: Deg<f32>) -> Result<(), BotError> {
        self.send_pos(
            pos,
            Vector3::new(0.0, 0.0, 0.0),
            Deg(0.0),
            yaw,
            EnumSet::empty(),
        )
        .await
    }

    pub async fn respawn(&self) -> Result<(), BotError> {
        self.tx
            .send(&ToSrvPkt::Respawn)
            .await
            .map(|_| ())
            .map_err(|e| BotError::Net(e.to_string()))
    }

    pub async fn got_blocks(&self, blocks: Vec<Point3<i16>>) -> Result<(), BotError> {
        self.tx
            .send(&ToSrvPkt::GotBlocks { blocks })
            .await
            .map(|_| ())
            .map_err(|e| BotError::Net(e.to_string()))
    }

    pub async fn disconnect(&self) -> Result<(), BotError> {
        self.tx
            .send(&ToSrvPkt::Disco)
            .await
            .map(|_| ())
            .map_err(|e| BotError::Net(e.to_string()))
    }
}

fn node_top(by: i32) -> f32 {
    (by as f32 + 0.5) * BS
}

impl Physics {
    pub fn step(
        &mut self,
        pos: Point3<f32>,
        dt: f32,
        blocks: &HashSet<Point3<i16>>,
    ) -> Point3<f32> {
        if self.wish_dir.x != 0.0 || self.wish_dir.z != 0.0 {
            self.vel.x = self.wish_dir.x * self.walk_speed;

            self.vel.z = self.wish_dir.z * self.walk_speed;
        } else {
            self.vel.x = 0.0;
            self.vel.z = 0.0;
        }

        if self.want_jump && self.on_ground {
            self.vel.y = self.jump_speed;
            self.on_ground = false;
        }

        self.want_jump = false;

        if !self.on_ground {
            self.vel.y -= self.gravity * dt;

            let terminal = -180.0 * BS;

            if self.vel.y < terminal {
                self.vel.y = terminal;
            }
        }

        let mut next = pos + self.vel * dt;

        let max_coord = (i32::MAX as f32) / (100.0 * BS) - 1.0;

        next.x = next.x.clamp(-max_coord, max_coord);
        next.y = next.y.clamp(-max_coord, max_coord);
        next.z = next.z.clamp(-max_coord, max_coord);

        let hw = 0.3 * BS;

        let min_x = ((next.x - hw) / BS).floor() as i32;

        let max_x = ((next.x + hw) / BS).floor() as i32;

        let min_z = ((next.z - hw) / BS).floor() as i32;

        let max_z = ((next.z + hw) / BS).floor() as i32;

        if self.vel.y <= 0.0 {
            let from_by = (pos.y / BS).floor() as i32;

            let to_by = (next.y / BS).floor() as i32 - 1;

            let mut landed = false;

            'fall: for by in (to_by..=from_by).rev() {
                let top = node_top(by);

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
                            landed = true;

                            break 'fall;
                        }
                    }
                }
            }

            if !landed {
                self.on_ground = false;
            }
        }

        if self.vel.y > 0.0 {
            let head_y = next.y + 1.75 * BS;

            let by = (head_y / BS).ceil() as i32;

            let bottom = node_top(by - 1);

            if head_y >= bottom {
                for bx in min_x..=max_x {
                    for bz in min_z..=max_z {
                        let key = Point3::new(bx as i16, by as i16, bz as i16);

                        if blocks.contains(&key) {
                            self.vel.y = 0.0;

                            next.y = bottom - 1.75 * BS;

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

        if forward {
            dz -= 1.0;
        }

        if back {
            dz += 1.0;
        }

        if left {
            dx -= 1.0;
        }

        if right {
            dx += 1.0;
        }

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
        self.gravity = gravity * BS;
    }
}
