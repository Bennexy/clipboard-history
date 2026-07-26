use crate::{Shutdown, cursor_position};
use anyhow::Result;
use clip_common::messages::{GlobalEvent, ServerMessage, SocketMessage};
use egui::Context;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot,
};
use tracing::{error, info};

pub async fn run(
    shutdown: Shutdown,
    mut socket_message_reciever: Receiver<SocketMessage>,
    ui_message_sender: Sender<ServerMessage>,
    egui_context_rx: oneshot::Receiver<Context>,
) -> Result<()> {
    let mut shutdown_rx = shutdown.subscribe();

    let egui_context = match egui_context_rx.await {
        Ok(ctx) => ctx,
        Err(err) => {
            error!("Failed to startup ui_worker since the egui context was never recieved due to {}", err);
            shutdown.trigger();
            return Err(anyhow::Error::new(err));
        }
    };

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("ui worker shutting down gracefully");
                shutdown_ui(egui_context);
                break;
            }

            Some(socket_message) = socket_message_reciever.recv() => {
                let _ = handle_socket_message(&egui_context, &ui_message_sender, socket_message).await;
            }
        }
    }

    Ok(())
}

async fn handle_socket_message(
    egui_context: &Context,
    ui_message_sender: &Sender<ServerMessage>,
    socket_message: SocketMessage,
) -> Result<()> {
    info!("Recieved a message: {:?}", socket_message);
    match socket_message {
        SocketMessage::ClientMessage(_) => (),
        SocketMessage::GlobalEvent(event) => match event {
            GlobalEvent::ShowUi => show_ui(&egui_context),
        },
        SocketMessage::ServerMessage(server_message) => {
            ui_message_sender
                .send(server_message)
                .await
                .inspect_err(|err| {
                    tracing::error!(
                        "Failed to send the recieved server message from the ui_worker to the main window due to {}.",
                        err
                    );
                })
                .map(|_| egui_context.request_repaint())?;
        }
    };

    Ok(())
}

fn show_ui(egui_context: &Context) {
    egui_context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    egui_context.send_viewport_cmd(egui::ViewportCommand::Focus);
    egui_context.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
    egui_context.request_repaint();
    // let (x, y) = cursor_position();
    // egui_context.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
}

fn shutdown_ui(egui_context: Context) {
    egui_context.send_viewport_cmd(egui::ViewportCommand::Close);
    egui_context.request_repaint();

    tracing::debug!("Requested close of the UI via the ui_worker!")
}
