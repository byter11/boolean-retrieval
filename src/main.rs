#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod model;

use std::{
    fs,
    path::PathBuf,
};

use eframe::egui;
use egui::RichText;
use model::{BooleanModel, Document};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(320.0, 240.0)),
        ..Default::default()
    };
    eframe::run_native(
        "Boolean Retrieval",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    )
}

#[derive(Default)]
struct MyApp {
    query: String,
    result: Vec<Document>,
    title: String,
    text: String,
    picked_path: Option<String>,
    model: BooleanModel,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.label("Choose docs location");

            if ui.button("Open Folder…").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.picked_path = Some(path.display().to_string());
                    match &self.picked_path {
                        Some(path) => {
                            self.model = BooleanModel::new();
                            self.model
                                .index(PathBuf::from(path), PathBuf::from("/dev/null"))
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
                let link = ui.link(&doc.name).on_hover_text(doc.summary.clone() + "...");
                if link.clicked() {
                    if let Some(picked_path) = &self.picked_path {
                        let text = fs::read_to_string(PathBuf::from(picked_path).join(&doc.name));
                        match text {
                            Ok(text) => {
                                self.title = String::from(&doc.name);
                                self.text = text;
                            },
                            _ => {}
                        }
                    }
                    link.highlight();
                }
            }
        });

        // Document preview panel
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading(&self.title);
                ui.label(RichText::new(&self.text));
            })
        });
    }
}
