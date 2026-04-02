// Use cgmath types from mt_net to avoid version mismatch
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
    Hp { hp: u16 },
    PlayerList {
        update_type: PlayerListUpdateType,
        players:     HashSet<String>,
    },
    TimeOfDay { time: u16, speed: f32 },
    Kicked(String),
    Disconnected,
}
