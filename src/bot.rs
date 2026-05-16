use mt_net::{
    enumset::EnumSet, CltSender, Deg, Key, PlayerPos, Point3, Rad, SenderExt, ToSrvPkt, Vector3,
    CONTENT_AIR, CONTENT_IGNORE, CONTENT_UNKNOWN,
};

use crate::{config::Config, error::BotError, event::Event, net, state::BotState};

pub const BS: f32 = 10.0;
pub const GRAVITY: f32 = 9.81 * BS;

fn is_collidable_node(id: u16) -> bool {
    // Some servers encode air as 0 in mapblock param0, while the protocol
    // also reserves explicit IDs for air/ignore/unknown. Treat all of those as
    // empty so the bot does not think every air node is solid ground.
    !matches!(id, 0 | CONTENT_AIR | CONTENT_IGNORE | CONTENT_UNKNOWN)
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
