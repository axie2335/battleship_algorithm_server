use algorithmplayer::algorithmplayer::AlgorithmPlayer;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serverinfo;
use serverinfo::data::coord::Coord;
use serverinfo::data::gamesetup::GameSetup;
use serverinfo::data::gamestate::CurrentGameState;
use serverinfo::data::gamestate::CurrentGameState::{Draw, Loss, Ongoing, Win};
use serverinfo::data::report::Report;
use serverinfo::data::shipinfo::ShipInfo;
use serverinfo::data::shots::{ShotRequest, Shots};
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::exit;

fn main() {
    let server_stream = connect_to_server_stream();

    let mut reader = BufReader::new(server_stream.try_clone().unwrap());
    let gamesetup: GameSetup = get_data_from_server::<GameSetup>(&mut reader).unwrap();

    let playerinfo = AlgorithmPlayer::new("player1".to_string(), gamesetup);
    let player = playerinfo.0;
    let ship_info = playerinfo.1;
    report_data_to_server::<ShipInfo>(&server_stream, &ship_info);
    player.draw_own_board();
    begin_game_loop(&server_stream, &mut reader, player);
}

fn connect_to_server_stream() -> TcpStream {
    println!("Enter the address to connect to:");

    let mut server_address = String::new();
    match io::stdin().read_line(&mut server_address) {
        Ok(_) => (),
        Err(_) => {
            println!("Failed to read line");
            exit(1);
        }
    }
    let server_address = server_address.trim();

    TcpStream::connect(server_address).expect("Failed to connect")
}

fn begin_game_loop(
    server_stream: &TcpStream,
    reader: &mut BufReader<TcpStream>,
    mut player: AlgorithmPlayer,
) {
    let mut game_state: CurrentGameState;
    loop {
        game_state = get_data_from_server::<CurrentGameState>(reader).unwrap();
        match game_state {
            Win => break,
            Loss => break,
            Draw => break,
            Ongoing => (),
        }
        // AlgorithmPlayer calculates this on its own
        let _ = get_data_from_server::<ShotRequest>(reader).unwrap();
        let shots = player.take_shots();
        let mut json_shots: Vec<Coord> = Vec::with_capacity(shots.len());
        for shot in shots {
            json_shots.push(Coord {
                x: shot.x,
                y: shot.y,
            });
        }
        let response: Shots = Shots { shots: json_shots };
        report_data_to_server::<Shots>(&server_stream, &response);
        let report = get_data_from_server::<Report>(reader).unwrap();
        let mut damaged_coords: Vec<Coord> = Vec::with_capacity(report.coords_damaged.len());
        for shot in report.coords_damaged {
            damaged_coords.push(Coord {
                x: shot.x,
                y: shot.y,
            });
        }
        let mut successful_hits: Vec<Coord> = Vec::with_capacity(report.shots_hit.len());
        for shot in report.shots_hit {
            successful_hits.push(Coord {
                x: shot.x,
                y: shot.y,
            });
        }
        player.report_damage(damaged_coords);
        player.record_successful_hits(successful_hits);
    }
    match game_state {
        Win => println!("WIN"),
        Loss => println!("LOSS"),
        Draw => println!("DRAW"),
        _ => (),
    }
}

fn get_data_from_server<T: DeserializeOwned>(
    reader: &mut BufReader<TcpStream>,
) -> Result<T, io::Error> {
    loop {
        let mut buffer = String::new();
        match reader.read_line(&mut buffer) {
            Ok(0) => {
                println!("Server closed");
                exit(0);
            }
            Ok(_) => match serde_json::from_str::<T>(&buffer) {
                Ok(report) => return Ok(report),
                Err(e) => return Err(e.into()),
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(e),
        }
    }
}

fn report_data_to_server<T: Serialize>(mut stream: &TcpStream, data: &T) {
    let data = serde_json::to_string(data).unwrap();
    let write_data = format!("{}\n", data);
    let _ = stream.write_all(write_data.as_bytes());
    let _ = stream.flush();
}
