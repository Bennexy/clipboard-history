#![allow(dead_code, unused_imports, unused_variables)]

use std::{sync::atomic::AtomicU64, time::Duration};

use eframe::egui;
use enigo::{Enigo, Mouse, Settings};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time::Instant,
};

pub mod socket;
use crate::socket::{ClientCommand, ClientResponse, ClipboardEntry};

struct SearchInput {
    search_string: String,
    last_input: Option<Instant>,
}

impl SearchInput {
    fn new() -> Self {
        Self { search_string: String::new(), last_input: None }
    }

    fn debounce_expired(&self) -> bool {
        self.last_input.is_some_and(|v| v.elapsed() >= Duration::from_millis(100))
    }
}

struct ClipstashApp {
    entries: Vec<ClipboardEntry>,
    search: SearchInput, // input: Option<SearchInput>,

    command_tx: Sender<ClientCommand>,
    response_rx: Receiver<ClientResponse>,

    initialized: bool,
    search_focused: bool,
    command_id_counter: usize,
}

impl ClipstashApp {
    fn new(tx: Sender<ClientCommand>, rx: Receiver<ClientResponse>) -> Self {
        Self {
            entries: Vec::with_capacity(50),
            search: SearchInput::new(),
            command_tx: tx,
            response_rx: rx,
            initialized: false,
            search_focused: false,
            command_id_counter: 0,
        }
    }

    fn initialize(&mut self) {
        let _ =
            self.command_tx.try_send(ClientCommand::GetAllClipboard { request_id: self.command_id_counter, limit: 50 });
        self.command_id_counter += 1;
        self.initialized = true;
    }
}

fn cursor_position() -> (f32, f32) {
    let enigo = Enigo::new(&Settings::default()).unwrap();

    let (x, y) = enigo.location().unwrap();

    (x as f32, y as f32)
}

impl eframe::App for ClipstashApp {
    fn ui(&mut self, ctx: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if !self.initialized {
            self.initialize();
        }

        while let Ok(response) = self.response_rx.try_recv() {
            self.entries = response.data;
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
                let _ = self.command_tx.try_send(ClientCommand::SearchTextClipboard {
                    request_id: self.command_id_counter,
                    limit: 50,
                    query,
                });
                self.command_id_counter += 1;
                self.search.last_input = None;
            }

            ctx.separator();
            let available_width = ctx.available_width();
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ctx, |ui| {
                for entry in &self.entries {
                    let response = ui.add_sized(
                        [available_width, 30.0],
                        egui::Button::selectable(false, entry.text.as_deref().unwrap_or("Missing!")),
                    );
                    // let _ = ui.selectable_label(false, entry.text.clone().unwrap_or("Missing!".into()));
                    ui.separator();
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let (app_tx, app_rx) = mpsc::channel(100);
    let (socket_tx, socket_rx) = mpsc::channel(100);

    let state = ClipstashApp::new(app_tx, socket_rx);

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.spawn(socket::run(app_rx, socket_tx));

    let (x, y) = cursor_position();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 400.0])
            .with_position([x, y])
            .with_resizable(false)
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native("Clipstash", options, Box::new(|_cc| Ok(Box::new(state))))
}
