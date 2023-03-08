#![allow(
    clippy::borrow_interior_mutable_const,
    clippy::declare_interior_mutable_const
)]

use client::PlayerMirrorClient;
use rrplug::{
    bindings::{
        convar::{FCVAR_GAMEDLL, FCVAR_SPONLY},
        squirreldatatypes::SQObject,
    },
    sq_raise_error, sq_return_null,
    wrappers::{
        northstar::{EngineLoadType, PluginData, ScriptVmType},
        squirrel::{call_sq_function, compile_string},
        vector::Vector3,
    },
    OnceCell,
};
use rrplug::{call_sq_object_function, prelude::*};
use server::PlayerMirrorServer;
use shared::MirroringType;
use std::sync::RwLock;

mod client;
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
    if compile_string(sqvm, sq_functions, true, r#"
    for(int x = 0; x < 16; x++)
    {
        entity dummy = CreateExpensiveScriptMoverModel( $"models/humans/heroes/mlt_hero_jack.mdl", <0,0,0>, <0,0,0>, SOLID_VPHYSICS, -1 )
        dummy.kv.skin = PILOT_SKIN_INDEX_GHOST
        dummy.SetScriptName(x.tostring())
    }

    thread void function() 
    {
        for(;;)
        {
            MirrorPlayerRunFrame( GetPlayerArray()[0].GetOrigin(), void function( int index, vector pos )
            {
                entity dummy = GetEntByScriptName(index.tostring())
        
                dummy.NonPhysicsMoveTo( pos, 0.1, 0.000000000001, 0.0000000000001 )
            } )
            wait 0
        };
    }()

    
    "#).is_err() {
        sq_raise_error!("can't compile anything in ohio", sqvm, sq_functions);
    }

    sq_return_null!()
}

#[rrplug::sqfunction(VM=Server,ExportName=MirrorPlayerRunFrame)]
fn runframe(player_pos: Vector3, func_move_dummies: fn(i32, Vector3)) {
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

                    for (index, vector) in player_positions
                        .to_vec()
                        .iter()
                        .filter(|v| v != &&zero) // since we don't clear positions this should be ok
                        .enumerate()
                    {
                        let index = index as i32;
                        if let Err(err) = call_sq_object_function!(
                            sqvm,
                            sq_functions,
                            func_move_dummies,
                            index,
                            vector
                        ) {
                            err.log()
                        }
                    }
                };

                _ = s.push_position_to_streams(player_pos);

                _ = s.accept_connection(); // spams too many useless errors >:(
            }
        }
        MirroringType::Client(c) => {
            if c.is_connected() {
                c.get_other_positions();

                let zero = Vector3::from([0., 0., 0.]);

                for (index, vector) in c
                    .player_positons
                    .to_vec()
                    .iter()
                    .filter(|v| v != &&zero)
                    .enumerate()
                {
                    if vector == &player_pos {
                        continue;
                    }

                    let index = index as i32;
                    if let Err(err) = call_sq_object_function!(
                        sqvm,
                        sq_functions,
                        func_move_dummies,
                        index,
                        vector
                    ) {
                        err.log()
                    }
                }

                c.push_position(player_pos);
            }
        }
    }

    sq_return_null!()
}

entry!(PlayerMirror);
