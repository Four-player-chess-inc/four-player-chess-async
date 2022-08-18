use crate::{ChessClock, Game, Ingame, JoinErr};
use crate::{PlayerToServer, ServerToPlayer};
use four_player_chess::ident::Ident::{First, Fourth, Second, Third};
use four_player_chess::mv::move_or_capture::MoveOrCapture;
use four_player_chess::mv::Move;
use four_player_chess::position::Position;
use four_player_chess::state::State;
use std::collections::HashMap;
use std::time::Duration;

fn init_game() -> (Game, Ingame, Ingame, Ingame, Ingame) {
    let mut g = Game::new(ChessClock::default());
    let a = g.join(First).unwrap();
    let b = g.join(Second).unwrap();
    let c = g.join(Third).unwrap();
    let d = g.join(Fourth).unwrap();
    (g, a, b, c, d)
}

// currently 4 players only
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn join() {
    let mut game = Game::new(ChessClock::default());
    assert!(game.join(First).is_ok());
    assert!(game.join(Second).is_ok());
    assert!(game.join(Third).is_ok());
    assert!(game.join(Fourth).is_ok());
    assert!(matches!(game.join(Second), Err(JoinErr)));
}

// make sure that player send channel closed if game drop
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn channel_close_if_game_drop() {
    let (g, a, b, c, d) = init_game();
    assert!(a.tx.send(PlayerToServer::Surrender).is_ok());
    core::mem::drop(g);
    // wait a bit, while game is dropping
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(b.tx.send(PlayerToServer::Surrender).is_err());
}

// make sure that game send broadcast to all players on they status changed
// expect on Second player
//  CallToMove(First)
//  make First surrender
//  StateChange(First lost)
//  CallToMove(Second)
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn status_change() {
    let (g, mut a, mut b, mut c, mut d) = init_game();
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(matches!(
        b.rx.try_recv(),
        Ok(ServerToPlayer::CallToMove { who, timers }) if who == First
    ));
    assert!(a.tx.send(PlayerToServer::Surrender).is_ok());
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(
        matches!(b.rx.try_recv(), Ok(ServerToPlayer::StateChange(state)) if state == HashMap::from([(First, State::Lost)]))
    );
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(matches!(
        b.rx.try_recv(),
         Ok(ServerToPlayer::CallToMove { who, timers }) if who == Second
    ));
}

// all surrender instead Third, Third win
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn surrenders() {
    let (g, mut a, mut b, mut c, mut d) = init_game();

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(matches!(
        a.rx.try_recv(),
         Ok(ServerToPlayer::CallToMove { who, timers }) if who == First
    ));
    a.tx.send(PlayerToServer::Surrender);

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(
        matches!(a.rx.try_recv(), Ok(ServerToPlayer::StateChange(state)) if state == HashMap::from([(First, State::Lost)]))
    );
    assert!(matches!(
        a.rx.try_recv(),
         Ok(ServerToPlayer::CallToMove { who, timers }) if who == Second
    ));
    b.tx.send(PlayerToServer::Surrender);

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(
        matches!(a.rx.try_recv(), Ok(ServerToPlayer::StateChange(state)) if state == HashMap::from([(Second, State::Lost)]))
    );
    assert!(matches!(
        a.rx.try_recv(),
         Ok(ServerToPlayer::CallToMove { who, timers }) if who == Third
    ));
    c.tx.send(PlayerToServer::Move(Move::MoveOrCapture(MoveOrCapture {
        from: Position::d13,
        to: Position::d12,
    })));

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(matches!(
        a.rx.try_recv(),
        Ok(ServerToPlayer::Move(Move::MoveOrCapture(MoveOrCapture {
            from: Position::d13,
            to: Position::d12,
        })))
    ));
    assert!(matches!(
        a.rx.try_recv(),
         Ok(ServerToPlayer::CallToMove { who, timers }) if who == Fourth
    ));
    d.tx.send(PlayerToServer::Surrender);

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(
        matches!(a.rx.try_recv(), Ok(ServerToPlayer::StateChange(state)) if state == HashMap::from([(Fourth, State::Lost)]))
    );
    assert!(matches!(
        a.rx.try_recv(),
        Ok(ServerToPlayer::GameOver(Third))
    ));
    assert!(matches!(a.rx.try_recv(), Err(TryRecvError)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn timeouts() {
    let mut g = Game::new(ChessClock::new(Duration::from_secs(5), Duration::from_secs(10)));
    let a = g.join(First).unwrap();
    let b = g.join(Second).unwrap();
    let c = g.join(Third).unwrap();
    let d = g.join(Fourth).unwrap();
}
