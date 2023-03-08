use rrplug::prelude::wait;
use rrplug::wrappers::vector::Vector3;

use crate::client::PlayerMirrorClient;
use crate::server::PlayerMirrorServer;

mod client;
mod server;
mod shared;

// fn main() {
//     let mut client = PlayerMirrorClient::new();
//     client.connect("localhost:8081".to_owned()).unwrap();

//     let mut num_iter = 0..1000;

//     let zero = Vector3::from([0., 0., 0.]);

//     loop {
//         client.get_other_positions();

//         let next_x = match &mut num_iter.next() {
//             Some(x) => *x as f32,
//             None => {
//                 num_iter = 0..1000;
//                 1000.
//             }
//         };

//         client.push_position(Vector3::from([-934. + next_x, -1169., 260.]));

//         println!(
//             "{:?}",
//             client
//                 .player_positons
//                 .to_vec()
//                 .iter()
//                 .filter(|p| p != &&zero)
//                 .collect::<Vec<&Vector3>>()
//         )
//     }
// }

fn main() {
    let mut server = PlayerMirrorServer::new();
    server.bind("192.168.0.243:8081".to_owned()).unwrap();

    server.push_position_to_streams(Vector3::from([11356., -2619., -204.])).unwrap();

    let mut saved_pos: [Vector3; 16] = server.get_positions_from_streams().unwrap();

    server.push_position_to_streams(Vector3::from([11356., -2619., -204.])).unwrap();

    loop {
        let positions = server.get_positions_from_streams().unwrap();

        for (index, pos) in saved_pos
            .iter()
            .enumerate()
            .filter(|e| &positions[e.0] != e.1)
        {
            println!("{index} changed to {pos:?}");
        }

        saved_pos = positions;

        _ = server.accept_connection();

        wait(100);
    }
}
