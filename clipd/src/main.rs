// #![allow(dead_code, unused_imports, unused_variables)]

use clip_common::messages::ServerEvent;
use rusqlite::Result;
use tokio::{signal::unix::SignalKind, sync::broadcast, task::JoinHandle, time::Duration};
use tracing::{error, info};

pub mod clipboard;
pub mod db;
pub mod socket;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let (server_event_tx, _) = broadcast::channel::<ServerEvent>(1);

    let (clipboard_tx, clipboard_rx) = tokio::sync::mpsc::channel(2);
    let (clipboard_events_tx, clipboard_events_rx) = tokio::sync::mpsc::channel(2);
    let (socket_tx, socket_rx) = tokio::sync::mpsc::channel(2);

    let db_worker = tokio::spawn(db::run(shutdown_tx.subscribe(), clipboard_rx, socket_rx, server_event_tx.clone()));

    let socket_worker =
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
    handle_shutdown(db_worker, clipboard_worker, socket_worker).await
}

async fn handle_shutdown(
    mut db_worker: JoinHandle<anyhow::Result<()>>,
    mut clipboard_worker: JoinHandle<anyhow::Result<()>>,
    mut socket_worker: JoinHandle<anyhow::Result<()>>,
) -> anyhow::Result<()> {
    let shutdown_timeout = Duration::from_secs(1);
    info!("Recieved shutdown, stopping workers with a timeout of {:.2} seconds.", shutdown_timeout.as_secs_f32());

    let (db, clipboard, socket) = tokio::join!(
        tokio::time::timeout(shutdown_timeout, &mut db_worker),
        tokio::time::timeout(shutdown_timeout, &mut clipboard_worker),
        tokio::time::timeout(shutdown_timeout, &mut socket_worker),
    );

    let mut timed_out = false;

    if db.is_err() {
        error!("DB shutdown timed out.");
        db_worker.abort();
        let _ = db_worker.await;
        timed_out = true;
    }

    if clipboard.is_err() {
        error!("DB shutdown timed out.");
        clipboard_worker.abort();
        let _ = clipboard_worker.await;
        timed_out = true;
    }

    if socket.is_err() {
        error!("Socket shutdown timed out.");
        socket_worker.abort();
        let _ = socket_worker.await;
        timed_out = true;
    }

    if timed_out {
        error!("Forcing process termination.");
        std::process::exit(1);
    }

    let db_ok = db.is_ok_and(|db| report_worker_result("db", db));
    let clipboard_ok = clipboard.is_ok_and(|clipboard| report_worker_result("clipboard", clipboard));
    let socket_ok = socket.is_ok_and(|socket| report_worker_result("socket", socket));

    let is_gracefull_shutdown = db_ok && clipboard_ok && socket_ok;
    if !is_gracefull_shutdown {
        error!("Shutdown processed with one or more errors.");
        std::process::exit(1);
    }

    info!("Shutdown completed gracefully");
    Ok(())
}

pub fn report_worker_result(name: &str, result: Result<anyhow::Result<()>, tokio::task::JoinError>) -> bool {
    match result {
        Ok(Ok(())) => {
            tracing::debug!("{name} shutdown cleanly");
            true
        }

        Ok(Err(err)) => {
            tracing::debug!("{name} failed: {err:?}");
            false
        }

        Err(join_err) => {
            tracing::debug!("{name} panicked: {join_err}");
            false
        }
    }
}
