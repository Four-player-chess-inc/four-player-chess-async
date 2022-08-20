use four_player_chess::ident::Ident;
use crate::{PlayerToServer, ServerToPlayer};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;

#[derive(Debug)]
pub struct Ingame {
    pub ident: Ident,
    pub(crate) rx: UnboundedReceiver<ServerToPlayer>,
    pub(crate) tx: UnboundedSender<PlayerToServer>,
}

impl Ingame {
    pub fn split(
        self,
    ) -> (
        UnboundedSender<PlayerToServer>,
        UnboundedReceiverStream<ServerToPlayer>,
    ) {
        (self.tx, UnboundedReceiverStream::new(self.rx))
    }
}
