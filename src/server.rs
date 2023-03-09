use crate::shared::{DataPacket, Positions, WorkerMessage, SINGLE_PACKET_SIZE};
use rrplug::{log, prelude::wait, wrappers::vector::Vector3};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
};

#[derive(Debug)]
pub struct PlayerMirrorServer {
    pub player_positions: Arc<RwLock<Positions>>, // max 16 players
    listener: Option<TcpListener>,
    workers: Vec<ConnectionWorker>,
    sender: Mutex<Sender<WorkerMessage>>,
}

impl PlayerMirrorServer {
    pub fn new() -> Self {
        let v = Vector3::from((0., 0., 0.));

        let positions = Arc::new(RwLock::new([
            v, v, v, v, v, v, v, v, v, v, v, v, v, v, v, v,
        ]));

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        // set size of 15
        const SIZE: usize = 15;

        let mut workers = Vec::with_capacity(SIZE);
        for id in 0..(SIZE - 1) {
            workers.push(ConnectionWorker::new(
                id,
                receiver.clone(),
                positions.clone(),
            ))
        }

        Self {
            player_positions: positions,
            listener: None,
            workers,
            sender: Mutex::new(sender),
        }
    }

    pub fn bind(&mut self, address: String) -> Result<(), String> {
        if let Some(l) = self.listener.take() {
            drop(l);
        }

        match TcpListener::bind(address) {
            Ok(l) => {
                l.set_nonblocking(true).expect("cannot set non blocking"); // just so I wouldn't need to spin up another thread
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
                    _ = self.sender.lock().unwrap().send(WorkerMessage::Work(conn));
                }
                Err(err) => return Err(err.to_string()),
            }
        }
        Ok(())
    }

    pub fn get_positions_from_streams(&mut self) -> Result<Positions, &'static str> {
        let lock = self
            .player_positions
            .write()
            .or(Err("can't have locks in ohio"))?;

        let mut positions = *lock;

        positions[15] = Vector3::from([0., 0., 0.]); // this is the local player on the server

        Ok(positions)
    }

    pub fn push_position_to_streams(&self, local_pos: Vector3) -> Result<(), &'static str> {
        let mut lock = self
            .player_positions
            .write()
            .or(Err("can't have locks in ohio"))?;
        *lock.get_mut(15).unwrap() = local_pos; // ^ or try_write?

        Ok(())
    }
}

impl Drop for PlayerMirrorServer {
    fn drop(&mut self) {
        let lock_poision = self.player_positions.clone();

        thread::spawn(move || {
            let lock = lock_poision.write().unwrap();
            let thing = lock[0];
            _ = thing;
            panic!();
        }); // this will poision the lock making it invalid and forcing threads to stop

        let lock = self.sender.lock().unwrap();

        for worker in self.workers.iter() {
            if worker.thread.is_some() {
                _ = lock.send(WorkerMessage::Death);
            }
        }
    }
}

#[derive(Debug)]
struct ConnectionWorker {
    thread: Option<JoinHandle<()>>,
    id: usize,
}

impl ConnectionWorker {
    fn new(
        id: usize,
        jobs: Arc<Mutex<Receiver<WorkerMessage>>>,
        positions: Arc<RwLock<Positions>>,
    ) -> Self {
        Self {
            thread: Some(thread::spawn(move || {
                Self::job_handler(id, jobs, positions)
            })),
            id,
        }
    }

    fn job_handler(
        id: usize,
        jobs: Arc<Mutex<Receiver<WorkerMessage>>>,
        positions: Arc<RwLock<Positions>>,
    ) {
        loop {
            let message = jobs.lock().unwrap().recv().unwrap(); // should never panic if it does
                                                                // managing the error is needing or else the mutex might get poisoned

            let stream = match message {
                WorkerMessage::Work(stream) => stream,
                WorkerMessage::Death => break,
                _ => continue,
            };

            log::info!("connection created for {id}");

            match stream.set_nonblocking(false) {
                Ok(_) => log::info!("{id} is blocking"),
                Err(err) => {
                    log::error!("{id} is non blocking {err}");
                    continue;
                }
            }

            Self::work(id, stream, &positions);

            log::error!("connection terminated for {id}");
        }

        log::warn!("{id} worker was told to stop");
    }

    fn work(id: usize, mut stream: TcpStream, positions: &Arc<RwLock<Positions>>) {
        let zero = Vector3::from([0., 0., 0.]);

        loop {
            let mut buffer = vec![0; SINGLE_PACKET_SIZE];

            _ = stream.read(&mut buffer);

            let recvpacket: DataPacket = match bincode::deserialize(&buffer) {
                Ok(p) => p,
                Err(err) => {
                    log::error!("couldn't deserialize packet : {err}");
                    return;
                }
            };

            let mut player_positions = {
                let mut positions = match positions.write() {
                    Ok(p) => p,
                    Err(err) => {
                        log::error!("couldn't get lock : {err}");
                        return;
                    }
                };

                positions[id] = recvpacket.into();

                positions
                    .iter()
                    .copied()
                    .map(|p| p.into())
                    .collect::<Vec<DataPacket>>()
            };

            player_positions[id] = zero.into();

            let sendpackets = match bincode::serialize(&player_positions) {
                Ok(s) => s,
                Err(err) => {
                    log::error!("couldn't serialize packets : {err}");
                    return;
                }
            };

            _ = stream.write_all(&sendpackets);

            wait(100);
        }
    }
}

impl Drop for ConnectionWorker {
    fn drop(&mut self) {
        log::warn!("Shutting down worker {}", self.id);

        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
