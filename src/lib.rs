mod ingame;
mod players_states_diff;
#[cfg(test)]
mod tests;

use crate::ingame::Ingame;
use crate::players_states_diff::PlayersStatesDiff;
use crate::ServerToPlayer::{CallToMove, GameOver, StateChange, Surrender};
use four_player_chess::four_player_chess::FourPlayerChess;
use four_player_chess::ident::Ident;
use four_player_chess::ident::Ident::{First, Fourth, Second, Third};
use four_player_chess::mv::{MakeMoveError, Move};
use four_player_chess::state::State;
use futures::future::Either;
use futures::{future, FutureExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Clone)]
pub enum ServerToPlayer {
    CallToMove(Ident),
    MoveError { mv: Move, error: MakeMoveError },
    Surrender(Ident),
    GameOver(Ident),
    Move(Move),
    StateChange(HashMap<Ident, State>),
}

#[derive(Debug, Clone)]
pub enum PlayerToServer {
    Move(Move),
    Surrender,
}

#[derive(Debug)]
enum MoveWait {
    Move(Move),
    Surrender,
    RxDisconnected,
}

type PlayerRx = UnboundedReceiver<ServerToPlayer>;
type PlayerTx = UnboundedSender<PlayerToServer>;
type PlayerRxs = HashMap<Ident, PlayerRx>;
type PlayerTxs = HashMap<Ident, PlayerTx>;

type ServerRx = UnboundedReceiver<PlayerToServer>;
type ServerTx = UnboundedSender<ServerToPlayer>;
type ServerRxs = HashMap<Ident, ServerRx>;
type ServerTxs = HashMap<Ident, ServerTx>;

pub struct Game {
    players_tx: PlayerTxs,
    players_rx: PlayerRxs,
    all_joined_notify: Arc<Notify>,
}

#[derive(Debug, PartialEq)]
pub struct JoinErr;

impl Game {
    pub fn new() -> Game {
        let mut server_rx_hm = HashMap::new();
        let mut server_tx_hm = HashMap::new();
        let mut player_rx_hm = HashMap::new();
        let mut player_tx_hm = HashMap::new();
        for i in [First, Second, Third, Fourth] {
            let (server_tx, player_rx) = unbounded_channel();
            let (player_tx, server_rx) = unbounded_channel();
            player_rx_hm.insert(i, player_rx);
            server_tx_hm.insert(i, server_tx);
            server_rx_hm.insert(i, server_rx);
            player_tx_hm.insert(i, player_tx);
        }

        let all_joined_notify = Arc::new(Notify::new());

        let game = FourPlayerChess::new();

        tokio::spawn(Self::game_process(
            server_rx_hm,
            server_tx_hm,
            all_joined_notify.clone(),
            game,
        ));

        Game {
            players_tx: player_tx_hm,
            players_rx: player_rx_hm,
            all_joined_notify,
        }
    }

    pub fn join(&mut self, ident: Ident) -> Result<Ingame, JoinErr> {
        let player_tx = self.players_tx.remove(&ident).ok_or(JoinErr)?;
        let player_rx = self.players_rx.remove(&ident).unwrap();

        if self.players_tx.is_empty() {
            self.all_joined_notify.notify_one();
        }

        Ok(Ingame {
            tx: player_tx,
            rx: player_rx,
        })
    }

    async fn wait_permitted_move(
        game: Arc<Mutex<FourPlayerChess>>,
        rx: &mut ServerRx,
        tx: &ServerTx,
    ) -> MoveWait {
        while let Some(msg) = rx.recv().await {
            match msg {
                PlayerToServer::Move(m) => match game.lock().await.make_move(m.clone()) {
                    Ok(_) => return MoveWait::Move(m),
                    Err(e) => {
                        #[allow(unused_must_use)] {
                            tx.send(ServerToPlayer::MoveError { mv: m, error: e });
                        }
                    }
                },
                PlayerToServer::Surrender => return MoveWait::Surrender,
            }
        }
        MoveWait::RxDisconnected
    }

    async fn game_process(
        mut rx: ServerRxs,
        tx: ServerTxs,
        all_joined_notify: Arc<Notify>,
        game: FourPlayerChess,
    ) {
        all_joined_notify.notified().await;

        let g = Arc::new(Mutex::new(game));

        let l = g.lock().await;
        let mut players_states = l.get_players_states();
        let mut who_move_o = l.who_move_next();
        drop(l);

        while let Some(who_move) = who_move_o {
            Self::broadcast(&tx, CallToMove(who_move));

            let player_rx = rx.get_mut(&who_move).unwrap();
            let player_tx = tx.get(&who_move).unwrap();

            let wait_move = Self::wait_permitted_move(g.clone(), player_rx, player_tx);
            let timeout = tokio::time::sleep(Duration::from_secs(1));

            let x = future::select(wait_move.boxed(), timeout.boxed()).await;
            match x {
                // wait_move_result
                Either::Left((r, _)) => match r {
                    MoveWait::Move(m) => Self::broadcast(&tx, ServerToPlayer::Move(m)),
                    MoveWait::Surrender | MoveWait::RxDisconnected => {
                        g.lock().await.surrender();
                        Self::broadcast(&tx, Surrender(who_move));
                    }
                },
                // timeout
                Either::Right((_, _)) => {
                    g.lock().await.surrender();
                    Self::broadcast(&tx, Surrender(who_move));
                }
            }
            
            let l = g.lock().await;
            let new_players_states = l.get_players_states();
            who_move_o = l.who_move_next();
            drop(l);

            if let Some(diff) = new_players_states.diff(&players_states) {
                players_states = new_players_states;
                Self::broadcast(&tx, StateChange(diff));
            }

        }

        Self::broadcast(&tx, GameOver(g.lock().await.who_win().unwrap()));
    }

    fn broadcast(tx: &ServerTxs, msg: ServerToPlayer) {
        tx.iter().for_each(|(_, v)| {
            #[allow(unused_must_use)] {
                v.send(msg.clone());
            }
        });
    }
}
