#![allow(
    clippy::borrow_interior_mutable_const,
    clippy::declare_interior_mutable_const
)]

use rrplug::prelude::*;
use rrplug::{
    bindings::{
        convar::{FCVAR_GAMEDLL, FCVAR_SPONLY},
        squirreldatatypes::SQObject,
    },
    call_sq_object_function, sq_raise_error, sq_return_null,
    wrappers::{
        northstar::{EngineLoadType, PluginData, ScriptVmType},
        squirrel::{call_sq_function, compile_string},
        vector::Vector3,
    },
    OnceCell,
};
use std::sync::RwLock;
use {
    client::PlayerMirrorClient,
    inlined_squirrel::SQURRIEL_CODE,
    server::PlayerMirrorServer,
    shared::{MirroringType, PlayerInfo},
};

mod client;
mod inlined_squirrel;
mod server;
mod shared;

#[derive(Debug)]
pub struct PlayerMirror {
    mirrortype: OnceCell<RwLock<MirroringType>>,
}

impl Plugin for PlayerMirror {
    fn new() -> Self {
        Self {
            mirrortype: OnceCell::new(),
        }
    }

    fn initialize(&mut self, plugin_data: &PluginData) {
        plugin_data.register_sq_functions(info_runframe).unwrap();
        plugin_data
            .register_sq_functions(info_wait_for_full_startup)
            .unwrap();

        self.mirrortype
            .set(RwLock::new(
                MirroringType::Client(PlayerMirrorClient::new()),
            ))
            .expect("failed to create mirrortype");
    }

    fn main(&self) {}

    fn on_engine_load(&self, engine: EngineLoadType) {
        let engine = match engine {
            EngineLoadType::Engine(engine) => engine,
            EngineLoadType::EngineFailed => return,
            EngineLoadType::Server => return,
            EngineLoadType::Client => return,
        };

        let server: i32 = FCVAR_GAMEDLL.try_into().unwrap();
        let sponly: i32 = FCVAR_SPONLY.try_into().unwrap();

        _ = engine.register_concommand(
            "client_connect",
            client_connect,
            "makes a connection to the address as a client",
            sponly | server,
        );

        _ = engine.register_concommand(
            "server_setup",
            server_setup,
            "sets up a server on the specified address",
            sponly | server,
        );
    }

    fn on_sqvm_created(&self, sqvm_handle: &squirrel::CSquirrelVMHandle) {
        if sqvm_handle.get_context() != ScriptVmType::Server {
            return;
        }

        let sqvm = unsafe { sqvm_handle.get_sqvm() };
        let sqfunctions = SQFUNCTIONS.server.wait();

        if let Err(err) = compile_string(
            sqvm,
            sqfunctions,
            true,
            r#"
        thread void function() {
            wait 5
            WaitForFullStartup()
        }()
        "#,
        ) {
            err.log();
        }

        match call_sq_function(sqvm, sqfunctions, "IsMultiplayer()") {
            Ok(_) => {
                if unsafe { (sqfunctions.sq_getbool)(sqvm, 1) } == 1 {
                    _ = sq_raise_error!(
                        "this plugin doesn't work in mp".to_string(),
                        sqvm,
                        sqfunctions,
                        noreturn
                    )
                }
            }
            Err(err) => err.log(),
        }
    }
}

#[rrplug::concommand]
fn client_connect(command: CCommandResult) {
    let address = match command.args.get(0) {
        Some(arg) => arg.to_owned(),
        None => {
            log::error!("no args :skull:");
            return;
        }
    };

    let mut mirrortype = match PLUGIN.wait().mirrortype.wait().try_write() {
        Ok(mirrortype) => mirrortype,
        Err(err) => {
            log::error!("{err:?}");
            return;
        }
    };

    match &mut *mirrortype {
        MirroringType::Server(s) => {
            s.shutdown();

            log::info!("stoping server");
            log::info!("connecting to server");

            let mut client = PlayerMirrorClient::new();

            match client.connect(address) {
                Ok(_) => {
                    log::info!("connected to server")
                }
                Err(err) => {
                    log::error!("failed to connect : {err}");
                    return;
                }
            }

            *mirrortype = MirroringType::Client(client);
        }
        MirroringType::Client(c) => match c.connect(address) {
            Ok(_) => {
                log::info!("connected to server")
            }
            Err(err) => {
                log::error!("failed to connect : {err}")
            }
        },
    }
}

#[rrplug::concommand]
fn server_setup(command: CCommandResult) {
    let address = match command.args.get(0) {
        Some(arg) => arg.to_owned(),
        None => {
            log::error!("no args :skull:");
            return;
        }
    };

    let mut mirrortype = match PLUGIN.wait().mirrortype.wait().try_write() {
        Ok(mirrortype) => mirrortype,
        Err(err) => {
            log::error!("{err:?}");
            return;
        }
    };

    match &mut *mirrortype {
        MirroringType::Server(s) => match s.bind(address) {
            Ok(_) => log::info!("started new server"),

            Err(err) => {
                log::error!("failed to bind to address : {err}")
            }
        },
        MirroringType::Client(c) => {
            c.shutdown();

            log::info!("stopping connection");
            log::info!("starting new server");

            let mut server = PlayerMirrorServer::new();

            match server.bind(address) {
                Ok(_) => {}
                Err(err) => {
                    log::error!("failed to bind to address : {err}");
                    return;
                }
            }

            *mirrortype = MirroringType::Server(server)
        }
    }
}

#[rrplug::sqfunction(VM=Server,ExportName=WaitForFullStartup)]
fn wait_for_full_startup() {
    if compile_string(sqvm, sq_functions, true, SQURRIEL_CODE).is_err() {
        sq_raise_error!("can't compile anything in ohio", sqvm, sq_functions);
    }

    sq_return_null!()
}

#[rrplug::sqfunction(VM=Server,ExportName=MirrorPlayerRunFrame)]
fn runframe(
    player_pos: Vector3,
    player_viewangle: Vector3,
    action: i32,
    func_move_dummies: fn(i32, Vector3, Vector3, i32),
) {
    let mut mirrortype = match PLUGIN.wait().mirrortype.wait().try_write() {
        Ok(mirrortype) => mirrortype,
        Err(err) => {
            log::error!("{err:?}");
            sq_return_null!()
        }
    };

    match &mut *mirrortype {
        MirroringType::Server(s) => {
            if s.is_listening() {
                let player_positions = s.get_positions_from_streams();

                if let Ok(player_positions) = player_positions {
                    let zero = Vector3::from([0., 0., 0.]);

                    for (index, info) in player_positions
                        .to_vec()
                        .iter()
                        .filter(|v| v.get_position() != zero) // since we don't clear positions this should be ok
                        .enumerate()
                    {
                        let index = index as i32;
                        if let Err(err) = call_sq_object_function!(
                            sqvm,
                            sq_functions,
                            func_move_dummies,
                            index,
                            info.get_position(),
                            info.get_viewangle(),
                            info.action.clone() as i32
                        ) {
                            err.log()
                        }
                    }
                };

                _ = s.push_position_to_streams(PlayerInfo::new(
                    player_pos,
                    player_viewangle,
                    action.try_into().unwrap(),
                ));

                _ = s.accept_connection(); // spams too many useless errors >:(
            }
        }
        MirroringType::Client(c) => {
            if c.is_connected() {
                let player_positons = c.get_other_positions();

                let zero = Vector3::from([0., 0., 0.]);

                for (index, info) in player_positons
                    .to_vec()
                    .iter()
                    .filter(|v| v.get_position() != zero)
                    .enumerate()
                {
                    let sent_player_pos = info.get_position();
                    let sent_player_viewangle = info.get_viewangle();
                    let sent_action = info.action.clone() as i32;

                    if sent_player_pos == player_pos {
                        continue;
                    }

                    let index = index as i32;
                    if let Err(err) = call_sq_object_function!(
                        sqvm,
                        sq_functions,
                        func_move_dummies,
                        index,
                        sent_player_pos,
                        sent_player_viewangle,
                        sent_action
                    ) {
                        err.log()
                    }
                }

                if let Err(err) = c.push_position(PlayerInfo::new(
                    player_pos,
                    player_viewangle,
                    action.try_into().unwrap(),
                )) {
                    log::warn!("{err}");
                }
            }
        }
    }

    sq_return_null!()
}

entry!(PlayerMirror);
