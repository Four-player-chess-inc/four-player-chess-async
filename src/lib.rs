pub mod chess_clock;
mod ingame;
mod players_states_diff;
#[cfg(test)]
mod tests;

use crate::chess_clock::ChessClock;
use crate::ingame::Ingame;
use crate::players_states_diff::PlayersStatesDiff;
use crate::ServerToPlayer::{CallToMove, GameOver, StateChange};
use four_player_chess::four_player_chess::FourPlayerChess;
use four_player_chess::ident::Ident;
use four_player_chess::ident::Ident::{First, Fourth, Second, Third};
use four_player_chess::mv::{MakeMoveError, Move};
use four_player_chess::state::State;
use futures::future::Either;
use futures::{future, FutureExt};
use futures_util::SinkExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, PartialEq)]
pub struct Timers {
    pub fast: Duration,
    pub rest_of_time: Duration,
}

impl From<&mut ChessClock> for Timers {
    fn from(c: &mut ChessClock) -> Self {
        let t = c.timers();
        Timers {
            fast: t.0,
            rest_of_time: t.1,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ServerToPlayer {
    CallToMove { who: Ident, timers: Timers },
    MoveError { mv: Move, error: MakeMoveError },
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
type ServerTxs = HashMap<Ident, ServerTx>;

pub struct Game {
    players_tx: PlayerTxs,
    players_rx: PlayerRxs,
    all_joined_notify: Arc<Notify>,
    jh: JoinHandle<()>,
}

#[derive(Debug, PartialEq)]
pub struct JoinErr;

struct ServerRxClock {
    server_rx: ServerRx,
    clock: ChessClock,
}

impl Game {
    pub fn new(clock: ChessClock) -> Game {
        let mut server_tx_hm = HashMap::new();
        let mut server_rx_clock_hm = HashMap::new();
        let mut player_tx_hm = HashMap::new();
        let mut player_rx_hm = HashMap::new();
        for i in [First, Second, Third, Fourth] {
            let (server_tx, player_rx) = unbounded_channel();
            let (player_tx, server_rx) = unbounded_channel();
            player_tx_hm.insert(i, player_tx);
            player_rx_hm.insert(i, player_rx);
            server_tx_hm.insert(i, server_tx);
            server_rx_clock_hm.insert(
                i,
                ServerRxClock {
                    server_rx,
                    clock: clock.clone(),
                },
            );
        }

        let all_joined_notify = Arc::new(Notify::new());

        let game = FourPlayerChess::new();

        let jh = tokio::spawn(Self::game_process(
            server_tx_hm,
            server_rx_clock_hm,
            all_joined_notify.clone(),
            game,
        ));

        Game {
            players_tx: player_tx_hm,
            players_rx: player_rx_hm,
            all_joined_notify,
            jh,
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
                        #[allow(unused_must_use)]
                        {
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
        server_txs: ServerTxs,
        mut server_rx_clock: HashMap<Ident, ServerRxClock>,
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
            let server_tx = server_txs.get(&who_move).unwrap();
            let mut rxs_clocks = server_rx_clock.get_mut(&who_move).unwrap();
            let mut server_rx = &mut rxs_clocks.server_rx;
            let mut clock = &mut rxs_clocks.clock;

            Self::broadcast(
                &server_txs,
                CallToMove {
                    who: who_move,
                    timers: clock.into(),
                },
            );

            let wait_move = Self::wait_permitted_move(g.clone(), server_rx, server_tx);

            let x = future::select(wait_move.boxed(), clock.start().boxed()).await;
            clock.stop();

            match x {
                // wait_move_result
                Either::Left((r, _)) => match r {
                    MoveWait::Move(m) => Self::broadcast(&server_txs, ServerToPlayer::Move(m)),
                    MoveWait::Surrender | MoveWait::RxDisconnected => {
                        g.lock().await.surrender();
                    }
                },
                // timeout
                Either::Right((_, _)) => {
                    g.lock().await.surrender();
                }
            }

            let l = g.lock().await;
            let new_players_states = l.get_players_states();
            who_move_o = l.who_move_next();
            drop(l);

            if let Some(diff) = new_players_states.diff(&players_states) {
                players_states = new_players_states;
                Self::broadcast(&server_txs, StateChange(diff));
            }
        }

        Self::broadcast(&server_txs, GameOver(g.lock().await.who_win().unwrap()));
    }

    fn broadcast(tx: &ServerTxs, msg: ServerToPlayer) {
        tx.iter().for_each(|(_, v)| {
            #[allow(unused_must_use)]
            {
                v.send(msg.clone());
            }
        });
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        self.jh.abort();
    }
}
