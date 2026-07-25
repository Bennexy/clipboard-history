#![allow(dead_code, unused_imports, unused_variables)]

use std::time::{SystemTime, UNIX_EPOCH};

use arboard::Clipboard;
use rusqlite::{Connection, Result, params};
use tokio::{
    signal::unix::SignalKind,
    sync::{broadcast, mpsc::Receiver},
    time::{Duration, sleep},
};
use tracing::{debug, error, info, trace, warn};

use crate::{clipboard::ClipboardEvent, db::Event};

pub mod clipboard;
pub mod db;
pub mod socket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let (server_event_tx, _) = broadcast::channel::<Event>(1);

    let (clipboard_tx, clipboard_rx) = tokio::sync::mpsc::channel(2);
    let (clipboard_events_tx, clipboard_events_rx) = tokio::sync::mpsc::channel(2);
    let (socket_tx, socket_rx) = tokio::sync::mpsc::channel(2);

    let db_worker = tokio::spawn(db::run(shutdown_tx.subscribe(), clipboard_rx, socket_rx, server_event_tx.clone()));

    let socket_server =
        tokio::spawn(socket::run(shutdown_tx.subscribe(), socket_tx, clipboard_events_tx, server_event_tx.subscribe()));

    let clipboard_worker = tokio::spawn(clipboard::run(shutdown_tx.subscribe(), clipboard_tx, clipboard_events_rx));

    let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt())?;
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate())?;

    tokio::select! {
        _ = sigint.recv() => {
            tracing::info!("Received SIGINT");
        }

        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM");
        }
    }

    info!("Starting shutdown");
    let _ = shutdown_tx.send(());

    let (db, clipboard, socket) = tokio::join!(db_worker, clipboard_worker, socket_server);

    report_worker_result("db", db);
    report_worker_result("clipboard", clipboard);
    report_worker_result("socket", socket);

    info!("Shutdown completed gracefully");

    Ok(())
}

fn report_worker_result(name: &str, result: Result<anyhow::Result<()>, tokio::task::JoinError>) {
    match result {
        Ok(Ok(())) => {
            tracing::debug!("{name} shutdown cleanly");
        }

        Ok(Err(err)) => {
            tracing::error!("{name} failed: {err:?}");
        }

        Err(join_err) => {
            tracing::error!("{name} panicked: {join_err}");
        }
    }
}
