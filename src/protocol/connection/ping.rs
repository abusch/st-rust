use std::time::Duration;

use tokio::{
    sync::{mpsc, oneshot, watch},
    time::Instant,
};
use tracing::{debug, info, warn};

use crate::protocol::{AsyncTypedMessage, TypedMessage};
use crate::protos::Ping;

pub struct ConnectionPingReceiver {
    last_message_received: watch::Receiver<Instant>,
    receive_duration: Duration,
}

impl ConnectionPingReceiver {
    pub fn new(last_message_received: watch::Receiver<Instant>) -> Self {
        Self {
            last_message_received,
            receive_duration: Duration::from_secs(90),
        }
    }

    pub async fn run(&self) {
        info!("Starting connection Ping receiver...");
        let mut interval = tokio::time::interval(self.receive_duration / 2);
        loop {
            let tick_time = interval.tick().await;
            let last_received_time = self.last_message_received.borrow();
            if tick_time.duration_since(*last_received_time) >= self.receive_duration {
                // TODO timeout the connection!
                warn!("No ping received for more than 90 seconds! Closing the connection (TODO)");
            }
        }
    }
}

pub struct ConnectionPingSender {
    outbox: mpsc::Sender<AsyncTypedMessage>,
    last_msg_sent: watch::Receiver<Instant>,
    send_duration: Duration,
}

impl ConnectionPingSender {
    pub fn new(
        outbox: mpsc::Sender<AsyncTypedMessage>,
        last_msg_sent: watch::Receiver<Instant>,
    ) -> Self {
        Self {
            outbox,
            last_msg_sent,
            send_duration: Duration::from_secs(90),
        }
    }

    pub async fn run(&self) {
        info!("Starting connection Ping sender...");
        let mut interval = tokio::time::interval(self.send_duration / 2);
        loop {
            let tick_time = interval.tick().await;
            let last_sent_time = *self.last_msg_sent.borrow();
            if tick_time <= last_sent_time {
                // This can happen when we send the ClusterConfig message and
                // the PingSender task hasn't started yet. Just ignore it...
                continue;
            }
            if tick_time.duration_since(last_sent_time) >= self.send_duration {
                debug!("No message sent in the last 90 seconds. Sending Ping...");

                let (ping, done) = self.ping_msg();
                if let Err(err) = self.outbox.send(ping).await {
                    warn!(%err, "Failed to send Ping message");
                }
                done.await.unwrap_or_else(
                    |e| warn!(err=%e, "Error while waiting to be notified. Sender was dropped?"),
                );
            }
        }
    }

    fn ping_msg(&self) -> (AsyncTypedMessage, oneshot::Receiver<()>) {
        let (done_tx, done_rx) = oneshot::channel();
        (
            AsyncTypedMessage {
                msg: TypedMessage::Ping(Ping::default()),
                done: done_tx,
            },
            done_rx,
        )
    }
}
