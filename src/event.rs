use mt_net::{Deg, Point3, PlayerListUpdateType};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum Event {
    Joined,
    Chat {
        sender: String,
        text:   String,
    },
    MovePlayer {
        pos:   Point3<f32>,
        pitch: Deg<f32>,
        yaw:   Deg<f32>,
    },
    Hp { hp: u8 },
    PlayerList {
        update_type: PlayerListUpdateType,
        players:     HashSet<String>,
    },
    TimeOfDay { time: u16, speed: f32 },
    MovementParams {
        walk_speed:  f32,
        jump_speed:  f32,
        gravity:     f32,
    },
    Died,
    BlockData {
        pos:    Point3<i16>,
        param0: Vec<u16>,   // node IDs, len = 4096
    },
    Kicked(String),
    Disconnected,
}