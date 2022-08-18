use four_player_chess::ident::Ident;
use four_player_chess::ident::Ident::{First, Fourth, Second, Third};
use four_player_chess::mv::move_or_capture::MoveOrCapture;
use four_player_chess::mv::Move;
use four_player_chess::position::Position::{e2, e4};
use four_player_chess_async::chess_clock::ChessClock;
use four_player_chess_async::Game;
use four_player_chess_async::PlayerToServer;
use futures::future::join_all;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn player_process(ident: Ident, game: Arc<Mutex<Game>>) {
    let ingame = game.lock().await.join(ident).unwrap();

    let (tx, mut rx) = ingame.split();
    while let Some(msg) = rx.next().await {
        println!("{:?} <- {:?}", ident, msg);
        if ident == First {
            tx.send(PlayerToServer::Move(Move::MoveOrCapture(MoveOrCapture {
                from: e2,
                to: e4,
            })))
            .unwrap();
        }
    }
}

#[tokio::main]
async fn main() {
    let game = Arc::new(Mutex::new(Game::new(ChessClock::default())));

    let mut p = Vec::new();

    for i in [First, Second, Third, Fourth] {
        p.push(tokio::spawn(player_process(i, game.clone())));
    }

    join_all(p).await;
}
