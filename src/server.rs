use crate::shared::DataPacket;
use rrplug::{log, wrappers::vector::Vector3};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

#[derive(Debug)]
pub struct PlayerMirrorServer {
    pub player_positons: [Vector3; 15], // max 16 players
    listener: Option<TcpListener>,
    connections: Vec<Option<TcpStream>>,
}

impl PlayerMirrorServer {
    pub fn new() -> Self {
        let v = Vector3::from((0., 0., 0.));
        Self {
            player_positons: [v, v, v, v, v, v, v, v, v, v, v, v, v, v, v],
            listener: None,
            connections: Vec::new(),
        }
    }

    pub fn bind(&mut self, address: String) -> Result<(), String> {
        if let Some(l) = self.listener.take() {
            drop(l);
            for conn in self.connections.iter().filter_map(|c| c.as_ref()) {
                _ = conn.shutdown(std::net::Shutdown::Both);
            }
            self.connections.clear()
        }

        match TcpListener::bind(address) {
            Ok(l) => {
                l.set_nonblocking(true).expect("cannot set non blocking");
                self.listener = Some(l);
                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn is_listening(&self) -> bool {
        self.listener.is_some()
    }

    pub fn shutdown(&mut self) {
        if let Some(l) = self.listener.take() {
            drop(l);
            for conn in self.connections.iter().filter_map(|c| c.as_ref()) {
                _ = conn.shutdown(std::net::Shutdown::Both);
            }
            self.connections.clear()
        }
    }

    pub fn accept_connection(&mut self) -> Result<(), String> {
        for conn in self
            .listener
            .as_ref()
            .expect("someone forgot to handle an option")
            .incoming()
        {
            match conn {
                Ok(conn) => {
                    conn.set_nonblocking(true)
                        .expect("couldn't set no delay on stream");
                    self.connections.push(Some(conn))
                }
                Err(err) => return Err(err.to_string()),
            }
        }
        Ok(())
    }

    pub fn get_positions_from_streams(&mut self) {
        for (index, conn) in self
            .connections
            .iter_mut()
            .filter_map(|c| c.as_mut())
            .enumerate()
        {
            let mut buffer = vec![0; 16];

            _ = conn.read(&mut buffer); // usually just spews useless errors

            let position: DataPacket = bincode::deserialize(&buffer).expect("couldn't deserialize");

            self.player_positons[index] = position.into();
        }
    }

    pub fn push_positions_to_streams(&mut self, local_pos: Vector3) {
        let mut player_positions: Vec<DataPacket> =
            self.player_positons.iter().map(|p| (*p).into()).collect();

        player_positions.push(local_pos.into());

        for index in 0..self.connections.len() {
            if let Some(conn) = self.connections[index].as_mut() {
                let mut player_positions = player_positions.clone();
                player_positions[index] = Vector3::from([0.,0.,0.]).into();

                let positions = bincode::serialize(&player_positions).expect("couldn't serialize");

                if let Err(err) = conn.write(&positions) {
                    log::error!("{}", err.to_string());
                    self.connections[index].take();
                }
            }
        }
    }
}
