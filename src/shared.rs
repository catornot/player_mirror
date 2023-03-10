use crate::{client::PlayerMirrorClient, server::PlayerMirrorServer};
use rrplug::wrappers::vector::Vector3;
use serde::{Deserialize, Serialize};
use std::mem::transmute;

pub const VEC_PACKET_SIZE: usize = 512;
pub const SINGLE_PACKET_SIZE: usize = 32;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlayerInfo {
    pub position: SerializableVector3,
    pub viewangle: SerializableVector3,
    pub action: Action,
}

impl PlayerInfo {
    pub fn new(position: Vector3, viewangle: Vector3, action: Action) -> Self {
        Self {
            position: position.into(),
            viewangle: viewangle.into(),
            action,
        }
    }

    pub fn get_position(&self) -> Vector3 {
        self.position.clone().into()
    }

    pub fn get_viewangle(&self) -> Vector3 {
        self.viewangle.clone().into()
    }
}

impl Default for PlayerInfo {
    fn default() -> Self {
        Self::new(
            Vector3::from([0., 0., 0.]),
            Vector3::from([0., 0., 0.]),
            Action::Stand,
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SerializableVector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Vector3> for SerializableVector3 {
    fn from(value: Vector3) -> Self {
        unsafe { transmute(value) }
    }
}

#[allow(clippy::from_over_into)]
impl Into<Vector3> for SerializableVector3 {
    fn into(self) -> Vector3 {
        unsafe { transmute(self) }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[repr(i32)]
pub enum Action {
    Crouch,
    Run,
    Stand,
    Jump,
    WallrunRight,
    WallrunLeft,
    WallrunFront,
    WallrunBack,
}

impl From<i32> for Action {
    fn from(value: i32) -> Self {
        if value <= 7 {
            // this the max Action can be with WallrunBack == 7
            unsafe { std::mem::transmute(value) }
        } else {
            Self::Stand
        }
    }
}

#[derive(Debug)]
pub enum MirroringType {
    Server(PlayerMirrorServer),
    Client(PlayerMirrorClient),
}

pub type PlayerInfoArray = [PlayerInfo; 16];

pub enum WorkerMessage {
    Work(std::net::TcpStream),
    Death,
    EndJob,
}
