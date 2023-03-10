use crate::shared::{PlayerInfo, PlayerInfoArray, WorkerMessage, VEC_PACKET_SIZE};
use rrplug::prelude::wait;
use std::{
    io::{Read, Write},
    net::TcpStream,
    ops::Deref,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
};

#[derive(Debug)]
pub struct PlayerMirrorClient {
    pub player_positons: Arc<RwLock<PlayerInfoArray>>, // max 15 players
    connnected: bool,
    job_send: Mutex<Sender<WorkerMessage>>,
    pos_send: Mutex<Sender<PlayerInfo>>,
    worker: PacketWorker,
}

impl PlayerMirrorClient {
    pub fn new() -> Self {
        let info = PlayerInfo::default();

        let player_postions = (0..16)
            .map(|_| info.clone())
            .collect::<Vec<PlayerInfo>>()
            .try_into()
            .unwrap();

        let player_positions = Arc::new(RwLock::new(player_postions));

        let (job_send, job_recv) = mpsc::channel();
        let (pos_send, pos_recv) = mpsc::channel();

        let worker = PacketWorker::new(job_recv, player_positions.clone(), pos_recv);

        Self {
            player_positons: player_positions,
            connnected: false,
            job_send: Mutex::new(job_send),
            pos_send: Mutex::new(pos_send),
            worker,
        }
    }

    pub fn connect(&mut self, address: String) -> Result<(), String> {
        if self.connnected {
            self.job_send
                .lock()
                .expect("lock not acquired")
                .send(WorkerMessage::EndJob)
                .unwrap();
        }

        self.connnected = true;

        match TcpStream::connect(address) {
            Ok(conn) => {
                self.job_send
                    .lock()
                    .expect("lock not acquired")
                    .send(WorkerMessage::Work(conn))
                    .unwrap();
                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn shutdown(&mut self) {
        if self.connnected {
            self.job_send
                .lock()
                .expect("lock not acquired")
                .send(WorkerMessage::EndJob)
                .unwrap();
        }

        self.connnected = false
    }

    pub fn is_connected(&self) -> bool {
        self.connnected
    }

    pub fn get_other_positions(&self) -> PlayerInfoArray {
        self.player_positons.read().unwrap().deref().clone()
    }

    pub fn push_position(&self, info: PlayerInfo) -> Result<(), &'static str> {
        self.pos_send
            .lock()
            .expect("lock not acquired")
            .send(info)
            .or(Err("can't send stuff"))
    }
}

impl Drop for PlayerMirrorClient {
    fn drop(&mut self) {
        let lock_poision = self.player_positons.clone();

        thread::spawn(move || {
            #[allow(unused_variables)]
            let lock = lock_poision.write().unwrap();
            panic!();
        }); // this will poision the lock making it invalid and forcing thread to stop

        let lock = self.job_send.lock().unwrap();

        _ = lock.send(WorkerMessage::EndJob);
        _ = lock.send(WorkerMessage::Death);

        _ = self.worker;
    }
}

#[derive(Debug)]
struct PacketWorker {
    thread: Option<JoinHandle<()>>,
}

impl PacketWorker {
    fn new(
        jobs: Receiver<WorkerMessage>,
        positions: Arc<RwLock<PlayerInfoArray>>,
        local_positions_recv: Receiver<PlayerInfo>,
    ) -> Self {
        Self {
            thread: Some(thread::spawn(move || {
                Self::job_handler(jobs, positions, local_positions_recv)
            })),
        }
    }

    fn job_handler(
        jobs: Receiver<WorkerMessage>,
        positions: Arc<RwLock<PlayerInfoArray>>,
        local_positions_recv: Receiver<PlayerInfo>,
    ) {
        loop {
            let message = jobs.recv().unwrap(); // should never panic if it does
                                                // managing the error is needing or else the mutex might get poisoned

            let stream = match message {
                WorkerMessage::Work(stream) => stream,
                WorkerMessage::Death => break,
                _ => continue,
            };

            log::info!("connection created for Stream");

            match stream.set_nonblocking(false) {
                Ok(_) => log::info!("stream is blocking"),
                Err(err) => {
                    log::error!("stream is non blocking {err}");
                    continue;
                }
            }

            wait(10);

            Self::work(stream, &positions, &local_positions_recv, &jobs);

            log::error!("connection terminated for client");
        }

        log::warn!("worker was told to stop");
    }

    fn work(
        mut stream: TcpStream,
        positions: &Arc<RwLock<PlayerInfoArray>>,
        local_positions_recv: &Receiver<PlayerInfo>,
        termination_notice: &Receiver<WorkerMessage>,
    ) {
        let mut last_known_local_position: PlayerInfo = PlayerInfo::default();

        loop {
            if let Ok(WorkerMessage::EndJob) = termination_notice.try_recv() {
                return;
            }

            let local_pos = local_positions_recv
                .try_recv()
                .unwrap_or(last_known_local_position.clone());

            if last_known_local_position != local_pos {
                last_known_local_position = local_pos.clone();
            }

            {
                let sendpackets = match bincode::serialize(&local_pos) {
                    Ok(s) => s,
                    Err(err) => {
                        log::error!("couldn't serialize packets : {err}");
                        return;
                    }
                };

                match stream.write_all(&sendpackets) {
                    Ok(_) => {}
                    Err(err) => log::error!("failed to write all : {err}"),
                }
            }

            let mut buffer = vec![0; VEC_PACKET_SIZE];

            match stream.read(&mut buffer) {
                Ok(_) => {}
                Err(err) => {
                    log::error!("failed to read : {err}");
                    return;
                }
            }

            let recvpackets: Vec<PlayerInfo> = match bincode::deserialize(&buffer) {
                Ok(p) => p,
                Err(err) => {
                    log::error!("couldn't deserialize packet : {err}");
                    return;
                }
            };

            {
                let mut positions = match positions.write() {
                    Ok(p) => p,
                    Err(err) => {
                        log::error!("couldn't get lock : {err}");
                        return;
                    }
                };

                match recvpackets.try_into() {
                    Ok(p) => *positions = p,
                    Err(_) => log::error!("failed to set new positions"),
                }
            }

            wait(100);
        }
    }
}

impl Drop for PacketWorker {
    fn drop(&mut self) {
        log::warn!("Shutting down client connection thread");

        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
