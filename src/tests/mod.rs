use four_player_chess::ident::Ident::{First, Fourth, Second, Third};
use crate::{Game, JoinErr};
use crate::PlayerToServer;



// currently 4 players only
#[tokio::test]
async fn join_fifth() {
    let mut game = Game::new();
    assert!(game.join(First).is_ok());
    assert!(game.join(Second).is_ok());
    assert!(game.join(Third).is_ok());
    assert!(game.join(Fourth).is_ok());
    assert!(matches!(game.join(Second), Err(JoinErr)));
}

