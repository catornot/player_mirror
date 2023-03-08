use crate::{client::PlayerMirrorClient, server::PlayerMirrorServer};
use rrplug::wrappers::vector::Vector3;
use serde::{Deserialize, Serialize};
use std::mem::transmute;

pub const VEC_PACKET_SIZE: usize = 256;
pub const SINGLE_PACKET_SIZE: usize = 16;

#[derive(Serialize, Deserialize, Debug,Clone)]
pub struct DataPacket {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Vector3> for DataPacket {
    fn from(value: Vector3) -> Self {
        unsafe { transmute(value) }
    }
}

#[allow(clippy::from_over_into)]
impl Into<Vector3> for DataPacket {
    fn into(self) -> Vector3 {
        unsafe { transmute(self) }
    }
}

#[derive(Debug)]
pub enum MirroringType {
    Server(PlayerMirrorServer),
    Client(PlayerMirrorClient),
}
