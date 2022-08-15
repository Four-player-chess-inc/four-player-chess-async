use crate::{Game, Ingame, JoinErr};
use crate::{PlayerToServer, ServerToPlayer};
use four_player_chess::ident::Ident::{First, Fourth, Second, Third};
use four_player_chess::state::State;
use std::collections::HashMap;
use std::time::Duration;

fn init(game: &mut Game) -> (Ingame, Ingame, Ingame, Ingame) {
    (
        game.join(First).unwrap(),
        game.join(Second).unwrap(),
        game.join(Third).unwrap(),
        game.join(Fourth).unwrap(),
    )
}

fn dropit<T>(t: T) {}

// currently 4 players only
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn join() {
    let mut game = Game::new();
    assert!(game.join(First).is_ok());
    assert!(game.join(Second).is_ok());
    assert!(game.join(Third).is_ok());
    assert!(game.join(Fourth).is_ok());
    assert!(matches!(game.join(Second), Err(JoinErr)));
}

// make sure that player send channel closed if game drop
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn channel_close_if_game_drop() {
    let mut game = Game::new();
    let (a, b, c, d) = init(&mut game);
    assert!(a.tx.send(PlayerToServer::Surrender).is_ok());
    dropit(game);
    // wait a bit, while game is dropping
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(b.tx.send(PlayerToServer::Surrender).is_err());
}

// make sure that game send broadcast to all player on his status changed
// expect on Second player
//  CallToMove(First)
//  make First surrender
//  StateChange(First lost)
//  CallToMove(Second)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn status_change() {
    let mut game = Game::new();
    let (mut a, mut b, mut c, mut d) = init(&mut game);

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(matches!(
        b.rx.try_recv(),
        Ok(ServerToPlayer::CallToMove(First))
    ));

    assert!(a.tx.send(PlayerToServer::Surrender).is_ok());

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(
        matches!(b.rx.try_recv(), Ok(ServerToPlayer::StateChange(state)) if state == HashMap::from([(First, State::Lost)]))
    );

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(
        matches!(b.rx.try_recv(), Ok(ServerToPlayer::CallToMove(Second)))
    );
}
