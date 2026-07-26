// #![allow(dead_code, unused_imports, unused_variables)]

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use eframe::egui;
use egui::Vec2;
use enigo::{Enigo, Mouse, Settings};
use tokio::{
    signal::unix::SignalKind,
    sync::{
        broadcast,
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    time::Instant,
};
use tracing::{debug, error, info, trace};

pub mod socket;
pub mod ui_worker;
use clip_common::{
    messages::{ClientRequest, ServerEvent, ServerMessage, ServerResponse},
    model::ClipboardEntry,
};

struct SearchInput {
    search_string: String,
    last_input: Option<Instant>,
}

enum NotificationKind {
    Success,
    Error,
}

struct Notification {
    kind: NotificationKind,
    text: String,
    expires: Option<Instant>,
}

impl SearchInput {
    fn new() -> Self {
        Self { search_string: String::new(), last_input: None }
    }

    fn debounce_expired(&self) -> bool {
        self.last_input.is_some_and(|v| v.elapsed() >= Duration::from_millis(50))
    }
}

#[derive(Clone)]
pub struct Shutdown {
    triggered: Arc<AtomicBool>,
    tx: broadcast::Sender<()>,
}

impl Shutdown {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(1);
        Self { triggered: Arc::new(AtomicBool::new(false)), tx }
    }

    fn trigger(&self) {
        if self.triggered.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            let _ = self.tx.send(());
            debug!("Triggered a shutdown!");
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }
}

struct ClipstashApp {
    entries: VecDeque<ClipboardEntry>,
    search: SearchInput, // input: Option<SearchInput>,

    shutdown: Shutdown,
    command_tx: Sender<ClientRequest>,
    response_rx: Receiver<ServerMessage>,

    initialized: bool,
    search_focused: bool,
    command_id_counter: usize,
    has_been_focused: bool,
    startup: Instant,
    notification: Option<Notification>,
}

impl ClipstashApp {
    fn new(shutdown: Shutdown, tx: Sender<ClientRequest>, rx: Receiver<ServerMessage>, startup: Instant) -> Self {
        Self {
            entries: VecDeque::with_capacity(50),
            search: SearchInput::new(),
            shutdown,
            command_tx: tx,
            response_rx: rx,
            initialized: false,
            search_focused: false,
            has_been_focused: false,
            command_id_counter: 0,
            startup,
            notification: None,
        }
    }

    fn initialize(&mut self) {
        info!("init start after: {}ms", self.startup.elapsed().as_millis());
        let _ = self.command_tx.try_send(ClientRequest::GetClipboardHistory { limit: 5000 });
        self.command_id_counter += 1;
        self.initialized = true;
        info!("init done after: {}ms", self.startup.elapsed().as_millis());
    }
}

fn cursor_position() -> (f32, f32) {
    let enigo = Enigo::new(&Settings::default()).unwrap();

    let (x, y) = enigo.location().unwrap();

    (x as f32, y as f32)
}

impl eframe::App for ClipstashApp {
    fn on_exit(&mut self) {
        self.shutdown.trigger();
        trace!("UI is shutting down gracefully");
    }

    fn ui(&mut self, ctx: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let init_done = self.initialized;
        if !self.initialized {
            info!("startup and starting render loop after: {}ms", self.startup.elapsed().as_millis());
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.has_been_focused = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.initialized = false;
        }

        if ctx.input(|i| i.viewport().focused == Some(true)) {
            self.has_been_focused = true;
        }

        if self.has_been_focused && ctx.input(|i| i.viewport().focused == Some(false)) {
            self.has_been_focused = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.initialized = false;
        }

        if !self.initialized {
            info!("startup and requesting data after: {}ms", self.startup.elapsed().as_millis());
            self.initialize();
        }

        while let Ok(response) = self.response_rx.try_recv() {
            match response {
                ServerMessage::Event(event) => match event {
                    ServerEvent::NewClipboardEntry(entry) => self.entries.push_front(entry),
                },
                ServerMessage::Response(server_response) => match server_response {
                    ServerResponse::ClipboardEntries(entries) => self.entries = entries.into(),
                    ServerResponse::Success => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        self.notification = Some(Notification {
                            kind: NotificationKind::Success,
                            text: "Clipboard updated".into(),
                            expires: Some(Instant::now() + Duration::from_secs(1)),
                        })
                    }
                    ServerResponse::Error(error) => {
                        self.notification = Some(Notification {
                            kind: NotificationKind::Error,
                            text: error,
                            expires: None, // stays until next message
                        })
                    }
                },
            };
            info!("startup and recieved data after: {}ms", self.startup.elapsed().as_millis())
        }

        egui::CentralPanel::default().show(ctx, |ctx| {
            // ui.heading("Clipboard History");

            let response = ctx.text_edit_singleline(&mut self.search.search_string);

            if !self.search_focused {
                response.request_focus();
                self.search_focused = true;
            }

            if response.changed() {
                self.search.last_input = Some(Instant::now());
            }

            if self.search.debounce_expired() {
                let query = self.search.search_string.clone();
                let _ = self.command_tx.try_send(ClientRequest::SearchClipboardHistory { limit: 50, query });
                self.command_id_counter += 1;
                self.search.last_input = None;
            }

            ctx.separator();
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show_rows(
                ctx,
                30.0,
                self.entries.len(),
                |ui, row_range| {
                    for i in row_range {
                        let entry = &self.entries[i];

                        let button = ui.add_sized(
                            [ui.available_width(), 30.0],
                            egui::Button::selectable(false, entry.text.clone()),
                        );

                        if button.clicked() {
                            let _ = self.command_tx.try_send(ClientRequest::SetClipboardEntry { id: entry.id });
                        }
                    }
                },
            );
        });

        if let Some(notification) = &self.notification
            && let Some(expires) = notification.expires
        {
            if Instant::now() >= expires {
                self.notification = None;
            } else {
                ctx.request_repaint_after(expires - Instant::now());
            }
        }

        let mut close_notification = false;

        if let Some(notification) = &self.notification {
            let fill = match notification.kind {
                NotificationKind::Success => egui::Color32::from_rgb(40, 160, 40), // todo: in the future i will just hide this box on success
                NotificationKind::Error => egui::Color32::from_rgb(180, 40, 40),
            };

            egui::Area::new("notification".into()).anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0]).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(fill).corner_radius(6.0).inner_margin(egui::Margin::same(8)).show(
                    ui,
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&notification.text).color(egui::Color32::WHITE));

                            if matches!(notification.kind, NotificationKind::Error) {
                                ui.add_space(8.0);

                                let button = ui.add_sized(
                                    Vec2::splat(16.0),
                                    egui::Button::new(egui::RichText::new("✕").color(egui::Color32::WHITE).strong())
                                        .frame(false),
                                );

                                if button.clicked() {
                                    close_notification = true;
                                }
                            }
                        });
                    },
                );
            });
        }

        if close_notification {
            self.notification = None;
        }

        if !init_done {
            info!("startup done after: {}ms", self.startup.elapsed().as_millis());
        }
    }
}

fn main() -> anyhow::Result<()> {
    let startup = Instant::now();
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let (app_tx, app_rx) = mpsc::channel(1);
    let (socket_tx, socket_rx) = mpsc::channel(1);
    let (ui_message_tx, ui_message_rx) = mpsc::channel(1);
    let shutdown = Shutdown::new();

    trace!("setup channels after: {:.2}ms", startup.elapsed().as_nanos() as f64 / 1_000_000.0);

    let runtime = tokio::runtime::Runtime::new().unwrap();

    let socket_worker: tokio::task::JoinHandle<Result<(), anyhow::Error>> =
        runtime.spawn(socket::run(shutdown.subscribe(), app_rx, socket_tx));

    trace!("started socket process: {:.2}ms", startup.elapsed().as_nanos() as f64 / 1_000_000.0);

    let (x, y) = cursor_position();

    trace!("got cursor position after: {:.2}ms", startup.elapsed().as_nanos() as f64 / 1_000_000.0);

    let (ctx_tx, ctx_rx) = oneshot::channel();
    let ui_worker = runtime.spawn(ui_worker::run(shutdown.clone(), socket_rx, ui_message_tx, ctx_rx));

    runtime.spawn(shutdown_handle(shutdown.clone(), socket_worker, ui_worker));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 400.0])
            .with_position([x, y])
            .with_resizable(false)
            .with_active(true)
            .with_taskbar(false)
            .with_decorations(false)
            .with_close_button(false)
            .with_maximize_button(false)
            .with_minimize_button(false)
            .with_movable_by_background(false)
            .with_title_shown(true)
            .with_titlebar_shown(true)
            .with_title("Clipboard histroy")
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "Clipstash",
        options,
        Box::new(|cc| {
            let state = ClipstashApp::new(shutdown.clone(), app_tx, ui_message_rx, startup);
            ctx_tx
                .send(cc.egui_ctx.clone())
                .expect("Failed to send the egui context to the ui_worker. This should never happen!");

            info!("starting the ui after: {:.2}ms", startup.elapsed().as_nanos() as f64 / 1_000_000.0);
            Ok(Box::new(state))
        }),
    )
    .map_err(anyhow::Error::new)?;

    info!("Shutdown processed gracefully.");
    Ok(())
}

async fn shutdown_handle(
    shutdown: Shutdown,
    mut socket_worker: tokio::task::JoinHandle<Result<(), anyhow::Error>>,
    mut ui_worker: tokio::task::JoinHandle<Result<(), anyhow::Error>>,
) -> anyhow::Result<()> {
    let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
    let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate()).unwrap();
    let mut shutdown_signal = shutdown.subscribe();
    tokio::select! {
        _ = shutdown_signal.recv() => {
            tracing::info!("Recieved shutdown signal via in app broadcast");
        }

        _ = sigint.recv() => {
            tracing::info!("Received SIGINT");
        }

        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM");
        }
    }

    shutdown.trigger();
    let shutdown_timeout = Duration::from_secs(1);
    info!("Recieved shutdown, stopping workers with a timeout of {:.2} seconds.", shutdown_timeout.as_secs_f32());

    let (socket, ui) = tokio::join!(
        tokio::time::timeout(shutdown_timeout, &mut socket_worker),
        tokio::time::timeout(shutdown_timeout, &mut ui_worker)
    );

    let mut timed_out = false;
    if socket.is_err() {
        error!("Socket shutdown timed out.");
        socket_worker.abort();
        let _ = socket_worker.await;
        timed_out = true;
    }

    if ui.is_err() {
        error!("UI shutdown timed out.");
        ui_worker.abort();
        let _ = ui_worker.await;
        timed_out = true;
    }

    if timed_out {
        error!("Forcing process termination.");
        std::process::exit(1);
    }

    let socket_ok = socket.is_ok_and(|socket| report_worker_result("socket", socket));
    let ui_ok = ui.is_ok_and(|ui| report_worker_result("ui", ui));

    let is_gracefull_shutdown = socket_ok && ui_ok;

    if !is_gracefull_shutdown {
        error!("Shutdown processed with one or more errors.");
        std::process::exit(1);
    }

    info!("Shutdown completed gracefully");
    std::process::exit(0); // end main process in case ui hasn't shut down yet.
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
