use rrplug::wrappers::vector::Vector3;

use crate::client::PlayerMirrorClient;

mod client;
mod server;
mod shared;

fn main() {
    let mut client = PlayerMirrorClient::new();
    client.connect("localhost:8081".to_owned()).unwrap();

    let mut num_iter = 0..1000;

    let zero = Vector3::from([0., 0., 0.]);

    loop {
        client.get_other_positions();

        let next_x = match &mut num_iter.next() {
            Some(x) => *x as f32,
            None => {
                num_iter = 0..1000;
                1000.
            }
        };

        client.push_position(Vector3::from([-934. + next_x, -1169., 260.]));

        println!(
            "{:?}",
            client
                .player_positons
                .to_vec()
                .iter()
                .filter(|p| p != &&zero)
                .collect::<Vec<&Vector3>>()
        )
    }
}
