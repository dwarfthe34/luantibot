//! Network layer: wraps mt_rudp + mt_net + mt_auth into a typed event stream.
//!
//! - `mt_net::connect()` → `(CltSender, CltReceiver, CltWorker)`
//! - Worker is spawned as its own task (UDP pump)
//! - `mt_auth::Auth` drives the SRP handshake via poll() + handle_pkt()
//! - `ReceiverExt::recv()` deserializes raw RUDP directly into `ToCltPkt`
//! - After AcceptAuth, auth sends Init2; we send CltReady and emit Joined

use mt_auth::Auth;
use mt_net::{
    connect, CltReceiver, CltSender, KickReason, ReceiverExt, SenderExt, ToCltPkt, ToSrvPkt,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::{config::Config, error::BotError, event::Event};

pub struct NetHandle {
    pub tx:       CltSender,
    pub event_rx: mpsc::Receiver<Event>,
}

pub async fn connect_bot(cfg: Config) -> Result<NetHandle, BotError> {
    let (tx, rx, worker) = connect(&cfg.address).await?;

    tokio::spawn(async move {
        worker.run().await;
    });

    let (event_tx, event_rx) = mpsc::channel(256);

    let auth = Auth::new(
        tx.clone(),
        cfg.username.clone(),
        cfg.password.clone(),
        cfg.lang.clone(),
    );

    tokio::spawn(recv_loop(rx, tx.clone(), auth, event_tx));

    Ok(NetHandle { tx, event_rx })
}

async fn recv_loop(
    mut rx: CltReceiver,
    tx: CltSender,
    mut auth: Auth,
    event_tx: mpsc::Sender<Event>,
) {
    loop {
        tokio::select! {
            _ = auth.poll() => {}

            pkt = rx.recv() => {
                match pkt {
                    None => {
                        let _ = event_tx.send(Event::Disconnected).await;
                        return;
                    }
                    Some(Err(e)) => {
                        debug!("recv/deserialize error (ignoring): {e}");
                        continue;
                    }
                    Some(Ok(pkt)) => {
                        handle_pkt(pkt, &tx, &mut auth, &event_tx).await;
                    }
                }
            }
        }
    }
}

async fn handle_pkt(
    pkt: ToCltPkt,
    tx: &CltSender,
    auth: &mut Auth,
    event_tx: &mpsc::Sender<Event>,
) {
    auth.handle_pkt(&pkt).await;

    match pkt {
        ToCltPkt::AcceptAuth { .. } => {
            info!("Auth accepted — sending CltReady");
            if let Err(e) = tx
                .send(&ToSrvPkt::CltReady {
                    major:    5,
                    minor:    7,
                    patch:    0,
                    reserved: 0,
                    version:  "luanti_bot 0.1.0".into(),
                    formspec: 4,
                })
                .await
                .map(|_| ())
            {
                warn!("CltReady send failed: {e}");
            }
            let _ = event_tx.send(Event::Joined).await;
        }

        ToCltPkt::Kick(reason) => {
            let msg = match reason {
                KickReason::Custom { custom } => custom,
                other => format!("{other:?}"),
            };
            warn!("Kicked: {msg}");
            let _ = event_tx.send(Event::Kicked(msg)).await;
        }

        ToCltPkt::LegacyKick { reason } => {
            warn!("Legacy kick: {reason}");
            let _ = event_tx.send(Event::Kicked(reason)).await;
        }

        ToCltPkt::ChatMsg { sender, text, .. } => {
            let _ = event_tx.send(Event::Chat { sender, text }).await;
        }

        // pos/pitch/yaw here are mt_net's cgmath 0.17 types, which match
        // our Event definition since event.rs also uses mt_net's re-exports
        ToCltPkt::MovePlayer { pos, pitch, yaw } => {
            let _ = event_tx.send(Event::MovePlayer { pos, pitch, yaw }).await;
        }

        ToCltPkt::Hp { hp, .. } => {
            let _ = event_tx.send(Event::Hp { hp }).await;
        }

        ToCltPkt::UpdatePlayerList { update_type, players } => {
            let _ = event_tx.send(Event::PlayerList { update_type, players }).await;
        }

        ToCltPkt::TimeOfDay { time, speed } => {
            let _ = event_tx.send(Event::TimeOfDay { time, speed }).await;
        }

        // Must ACK or server stops sending map data
        ToCltPkt::BlockData { pos, .. } => {
            let _ = tx.send(&ToSrvPkt::GotBlocks { blocks: vec![pos] }).await.map(|_| ());
        }

        _ => {}
    }
}
