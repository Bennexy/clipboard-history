// #![allow(dead_code, unused_imports, unused_variables)]

use std::{collections::VecDeque, time::Duration};

use eframe::egui;
use egui::Vec2;
use enigo::{Enigo, Mouse, Settings};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time::Instant,
};
use tracing::{event, info};

pub mod socket;
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
        self.last_input.is_some_and(|v| v.elapsed() >= Duration::from_millis(1))
    }
}

struct ClipstashApp {
    entries: VecDeque<ClipboardEntry>,
    search: SearchInput, // input: Option<SearchInput>,

    command_tx: Sender<ClientRequest>,
    response_rx: Receiver<ServerMessage>,

    initialized: bool,
    search_focused: bool,
    command_id_counter: usize,
    startup: Instant,
    notification: Option<Notification>,
}

impl ClipstashApp {
    fn new(tx: Sender<ClientRequest>, rx: Receiver<ServerMessage>, startup: Instant) -> Self {
        Self {
            entries: VecDeque::with_capacity(50),
            search: SearchInput::new(),
            command_tx: tx,
            response_rx: rx,
            initialized: false,
            search_focused: false,
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
    fn ui(&mut self, ctx: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let init_done = self.initialized;
        if !self.initialized {
            info!("startup and starting render loop after: {}ms", self.startup.elapsed().as_millis());
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                NotificationKind::Success => egui::Color32::from_rgb(40, 160, 40),
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
        //ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn main() -> eframe::Result<()> {
    let startup = Instant::now();
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let (app_tx, app_rx) = mpsc::channel(100);
    let (socket_tx, socket_rx) = mpsc::channel(100);

    info!("setup channels after: {}ms", startup.elapsed().as_millis());

    let state = ClipstashApp::new(app_tx, socket_rx, startup);

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.spawn(socket::run(app_rx, socket_tx));

    info!("started socket process: {}ms", startup.elapsed().as_millis());

    let (x, y) = cursor_position();

    info!("got cursor position after: {}ms", startup.elapsed().as_millis());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 400.0])
            .with_position([x, y])
            .with_resizable(true)
            .with_active(true)
            .with_minimize_button(false)
            .with_taskbar(false)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "Clipstash",
        options,
        Box::new(|_cc| {
            info!("starting the ui after: {}ms", startup.elapsed().as_millis());
            Ok(Box::new(state))
        }),
    )
}
