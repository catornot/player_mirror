use crate::shared::{DataPacket, VEC_PACKET_SIZE};
use rrplug::wrappers::vector::Vector3;
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[derive(Debug)]
pub struct PlayerMirrorClient {
    pub player_positons: [Vector3; 16], // max 15 players
    conn: Option<TcpStream>,
}

impl PlayerMirrorClient {
    pub fn new() -> Self {
        let v = Vector3::from((0., 0., 0.));
        Self {
            player_positons: [v, v, v, v, v, v, v, v, v, v, v, v, v, v, v, v],
            conn: None,
        }
    }

    pub fn connect(&mut self, address: String) -> Result<(), String> {
        if let Some(c) = self.conn.take() {
            drop(c);
        }

        match TcpStream::connect(address) {
            Ok(conn) => {
                conn.set_nonblocking(true).expect("cannot set non blocking");
                self.conn = Some(conn);
                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(c) = self.conn.take() {
            drop(c);
        }
    }

    pub fn is_connected(&self) -> bool {
        self.conn.is_some()
    }

    pub fn get_other_positions(&mut self) {
        let conn = self
            .conn
            .as_mut()
            .expect("someone forgot to handle an option");

        let mut buffer = vec![0; VEC_PACKET_SIZE];

        _ = conn.read(&mut buffer); // usually just spews useless errors

        let position: Vec<DataPacket> = match bincode::deserialize(&buffer) {
            Ok(packet) => packet,
            Err(err) => {
                log::warn!("server sent bad packet {err}");
                return;
            }
        };

        if position.len() < self.player_positons.len() {
            return;
        }

        self.player_positons = position
            .into_iter()
            .map(|p| p.into())
            .collect::<Vec<Vector3>>()
            .try_into()
            .expect("not the right lenght of Vec<DataPacket>");
    }

    pub fn push_position(&mut self, local_position: Vector3) {
        let conn = self
            .conn
            .as_mut()
            .expect("someone forgot to handle an option");

        let lp: DataPacket = local_position.into();

        let positions = bincode::serialize(&lp).expect("couldn't serialize");

        _ = conn.write(&positions); // usually just spews useless errors
    }
}
