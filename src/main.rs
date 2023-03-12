#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod model;

use std::path::PathBuf;

use eframe::egui;
use egui::RichText;
use model::{BooleanModel, Document, DocumentDetails};
use serde::{Deserialize, Serialize};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(1280.0, 720.0)),
        ..Default::default()
    };
    eframe::run_native(
        "Boolean Retrieval",
        options,
        Box::new(|_cc| Box::new(MyApp::new(_cc))),
    )
}

#[derive(Default, Serialize, Deserialize)]
struct MyApp {
    query: String,
    result: Vec<Document>,
    selected_document: DocumentDetails,
    saving: bool,
    can_close: bool,
    picked_path: Option<String>,
    model: BooleanModel,
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app =
            eframe::get_value(cc.storage.unwrap(), eframe::APP_KEY).unwrap_or(Self::default());
        app.saving = false;
        app.can_close = false;
        app
    }
}

impl eframe::App for MyApp {
    fn on_close_event(&mut self) -> bool {
        self.saving = true;
        self.can_close
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
        self.saving = false;
    }

    fn persist_egui_memory(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.label("Choose docs location");
            if ui.button("Open Folder…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.picked_path = Some(path.display().to_string());
                    match &self.picked_path {
                        Some(path) => {
                            self.model = BooleanModel::new();
                            self.model.index(PathBuf::from(path))
                        }
                        None => {}
                    }
                }
            }

            if let Some(picked_path) = &self.picked_path {
                ui.horizontal(|ui| {
                    ui.label("Documents directory:");
                    ui.monospace(picked_path);
                });
            }

            // Query Input
            ui.text_edit_singleline(&mut self.query);

            // Search button
            if ui.button("Search").clicked() {
                self.result = self.model.query_boolean(&self.query);
            }

            // Enter key handler
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                if self.query.contains("/") {
                    self.result = self.model.query_positional(&self.query);
                } else {
                    self.result = self.model.query_boolean(&self.query);
                }
            }

            // Render results with summary on hover
            for doc in &self.result {
                let details = self.model.get_doc(doc.id);

                if details.is_none() {
                    continue;
                }
                let details = details.unwrap();

                let link = ui
                    .link(&details.name)
                    .on_hover_text(details.summary.clone() + "...");

                if link.clicked() {
                    self.selected_document = details.to_owned();
                    link.highlight();
                }
            }
        });

        // Document preview panel
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.saving {
                ui.label("Saving model ⏳");
                self.can_close = true;
                _frame.close();
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading(&self.selected_document.name);
                    ui.label(RichText::new(&self.selected_document.text))
                });
            }
        });

        // Bottom panel with option to view th boolean model as json
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            if ui.link("View model").clicked() {
                self.selected_document = DocumentDetails {
                    name: String::from("Model JSON"),
                    summary: String::from("Model JSON"),
                    text: serde_json::to_string_pretty(&self.model).unwrap_or(String::from("")),
                }
            }
        });
    }
}
