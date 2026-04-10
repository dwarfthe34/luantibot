use mt_net::{Deg, Point3};
use crate::physics::Physics;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct BotState {
    pub pos:       Point3<f32>,
    pub pitch:     Deg<f32>,
    pub yaw:       Deg<f32>,
    pub hp:        u16,
    pub joined:    bool,
    pub respawned: bool,
    pub physics:   Physics,
    /// Known solid block positions from received BlockData packets
    pub blocks:    HashSet<Point3<i16>>,
}

impl Default for BotState {
    fn default() -> Self {
        Self {
            pos:       Point3::new(0.0, 0.0, 0.0),
            pitch:     Deg(0.0),
            yaw:       Deg(0.0),
            hp:        20,
            joined:    false,
            respawned: false,
            physics:   Physics::default(),
            blocks:    HashSet::new(),
        }
    }
}

