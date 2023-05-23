use crate::edit_distance::levenshtein_distance;
use egui::RichText;
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use poll_promise::Promise;
use std::path::PathBuf;
use std::thread;

use csv;
use rfd::FileDialog;

#[derive(Debug)]
struct Table {
    file: PathBuf,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct TableSettings {
    striped: bool,
    resizable: bool,
    scroll_to_row: Option<usize>,
}

impl Default for TableSettings {
    fn default() -> Self {
        Self {
            striped: true,
            resizable: true,
            scroll_to_row: Some(9),
        }
    }
}

enum LogLevel {
    Info,
    Warning,
    Error,
}

struct LogMessage {
    msg: String,
    level: LogLevel,
}

impl LogMessage {
    fn new(msg: String, level: LogLevel) -> Self {
        Self { msg, level }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct EditDistanceSettings {
    col_idx: usize,
    similarity: usize,
    case_sensitive: bool,
}

impl Default for EditDistanceSettings {
    fn default() -> Self {
        Self {
            col_idx: 0,
            similarity: 100,
            case_sensitive: true,
        }
    }
}

struct ResultWindow {
    open: bool,
    indices: Option<Promise<Vec<Vec<usize>>>>,
}
impl Default for ResultWindow {
    fn default() -> Self {
        Self {
            open: false,
            indices: None,
        }
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // Example stuff:
    label: String,

    // this how you opt-out of serialization of a member
    #[serde(skip)]
    value: f32,

    #[serde(skip)]
    table: Option<Table>,

    table_settings: TableSettings,

    edit_distance_settings: EditDistanceSettings,

    #[serde(skip)]
    logs: Vec<LogMessage>,

    #[serde(skip)]
    result_window: ResultWindow,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            table: None,
            table_settings: Default::default(),
            edit_distance_settings: Default::default(),
            logs: Vec::new(),
            result_window: Default::default(),
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        setup_custom_fonts(&cc.egui_ctx);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl TemplateApp {
    fn table_ui(&mut self, ui: &mut egui::Ui) {
        if self.table.is_none() {
            return;
        }
        let t = self.table.as_ref().unwrap();

        let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

        let mut table = TableBuilder::new(ui)
            .striped(self.table_settings.striped)
            .resizable(self.table_settings.resizable)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .min_scrolled_height(0.0);
        for _ in 0..t.headers.len() {
            table = table.column(Column::remainder());
        }

        if let Some(row_nr) = self.table_settings.scroll_to_row.take() {
            table = table.scroll_to_row(row_nr, None);
        }

        table
            .header(20.0, |mut header| {
                for col in &t.headers {
                    header.col(|ui| {
                        ui.strong(col);
                    });
                }
            })
            .body(|body| {
                let row_height = text_height * 1.2;
                body.rows(row_height, t.rows.len(), |idx, mut row| {
                    for col in &t.rows[idx] {
                        row.col(|ui| {
                            ui.label(col);
                        });
                    }
                })
            });
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        let file = FileDialog::new()
                            .add_filter("csv", &["csv"])
                            .set_directory("/")
                            .pick_file();

                        // parse
                        if let Some(file) = file {
                            match read_table(file) {
                                Ok(t) => {
                                    self.table = Some(t);
                                }
                                Err(e) => {
                                    // Failed to parse the csv file
                                    println!("Failed to parse the csv file, {:?}", e);
                                    self.logs.push(LogMessage::new(
                                        String::from("Failed to parse the csv file"),
                                        LogLevel::Error,
                                    ));
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if self.logs.len() > 0 {
                    let last_log = &self.logs[self.logs.len() - 1];
                    match last_log {
                        LogMessage {
                            level: LogLevel::Info,
                            msg,
                        } => {
                            ui.label(RichText::new(format!("ℹ️ {} ℹ️", msg)));
                        }
                        LogMessage {
                            level: LogLevel::Warning,
                            msg,
                        } => {
                            ui.label(
                                RichText::new(format!("⚠️ {} ⚠️", msg))
                                    .color(ui.visuals().warn_fg_color),
                            );
                        }
                        LogMessage {
                            level: LogLevel::Error,
                            msg,
                        } => {
                            ui.label(
                                RichText::new(format!("❌ {} ❌", msg))
                                    .color(ui.visuals().error_fg_color),
                            );
                        }
                    }
                    if ui.button("CONFIRM").clicked() {
                        self.logs.pop();
                    }
                } else {
                    ui.label(RichText::new(format!("ℹ️ {} ℹ️", "Loaded")));
                }
            });
            ui.add_space(10.0);
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Grid::new("table_settings")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Choose a csv file to open");
                    if ui.button("Open").clicked() {
                        let file = FileDialog::new()
                            .add_filter("csv", &["csv"])
                            .set_directory("/")
                            .pick_file();

                        // parse
                        if let Some(file) = file {
                            match read_table(file) {
                                Ok(t) => {
                                    self.table = Some(t);
                                }
                                Err(e) => {
                                    // Failed to parse the csv file
                                    println!("Failed to parse the csv file, {:?}", e);
                                    self.logs.push(LogMessage::new(
                                        String::from("Failed to parse the csv file"),
                                        LogLevel::Error,
                                    ));
                                }
                            }
                        }
                    }
                    ui.end_row();

                    ui.label("Table display settings");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.table_settings.striped, "Striped");
                        ui.checkbox(&mut self.table_settings.resizable, "Resizable columns");
                    });
                    ui.end_row();
                    if let Some(t) = &self.table {
                        // Show opened file path
                        ui.label("Opened file");
                        ui.label(format!(
                            "{}",
                            t.file.to_str().unwrap_or("Err when parse file path"),
                        ));
                        ui.end_row();

                        ui.label("Similarity");
                        ui.add(
                            egui::DragValue::new(&mut self.edit_distance_settings.similarity)
                                .clamp_range(0..=100)
                                .max_decimals(0)
                                .speed(1.0),
                        );
                        ui.end_row();

                        ui.label("CaseCensitive");
                        ui.checkbox(
                            &mut self.edit_distance_settings.case_sensitive,
                            "CaseSensitive",
                        );
                        ui.end_row();

                        ui.label("Column");
                        egui::ComboBox::from_label("LEN")
                            .selected_text(format!(
                                "{:?}",
                                t.headers[self.edit_distance_settings.col_idx]
                            ))
                            .show_ui(ui, |ui| {
                                ui.style_mut().wrap = Some(false);
                                ui.set_min_width(60.0);
                                for (idx, col_name) in t.headers.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut self.edit_distance_settings.col_idx,
                                        idx,
                                        col_name,
                                    );
                                }
                            });
                        ui.end_row();
                    }
                });

            if let Some(t) = &self.table {
                ui.horizontal(|ui| {
                    if ui.button("Cal similarity").clicked() {
                        let keys: Vec<String> = t
                            .rows
                            .iter()
                            .map(|v| v[self.edit_distance_settings.col_idx].to_owned())
                            .collect();

                        let ctx = ctx.clone();
                        let (sender, promise) = Promise::new();
                        self.result_window.indices = Some(promise);
                        let similarity = self.edit_distance_settings.similarity;
                        let case_sensitive = self.edit_distance_settings.case_sensitive;

                        thread::spawn(move || {
                            let res = group_by_similarity_v2(&keys, similarity, case_sensitive);
                            sender.send(res);
                            ctx.request_repaint();
                        });
                    }
                });
                if let Some(task) = &self.result_window.indices {
                    if task.ready().is_none() {
                        ui.label("Calculating...");
                        ui.spinner();
                    }
                }
            }

            egui::warn_if_debug_build(ui);

            ui.separator();

            StripBuilder::new(ui)
                .size(Size::remainder().at_least(100.0)) // for the table
                .size(Size::exact(10.0)) // for the source code link
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        egui::ScrollArea::horizontal().show(ui, |ui| {
                            self.table_ui(ui);
                        });
                    });
                    strip.cell(|ui| {
                        ui.label(
                            RichText::new("⚠ DEV ⚠")
                                .small()
                                .color(ui.visuals().warn_fg_color),
                        )
                        .on_hover_text("This software is WIP");
                    });
                });
        });

        if let Some(task) = &self.result_window.indices {
            if let Some(indices) = task.ready() {
                self.result_window.open = true;
                let mut window = egui::Window::new("Result")
                    .resizable(true)
                    .collapsible(true)
                    .title_bar(true)
                    .scroll2([true, true])
                    .enabled(true);
                window = window.open(&mut self.result_window.open);
                window.show(ctx, |ui| {
                    // Show result table
                    if self.table.is_none() {
                        return;
                    }
                    let t = self.table.as_ref().unwrap();

                    let text_height = egui::TextStyle::Body.resolve(ui.style()).size;

                    let mut table = TableBuilder::new(ui)
                        .striped(self.table_settings.striped)
                        .resizable(self.table_settings.resizable)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .min_scrolled_height(0.0);

                    // Index row
                    table = table.column(Column::auto());
                    for _ in 0..t.headers.len() {
                        table = table.column(Column::remainder());
                    }

                    if let Some(row_nr) = self.table_settings.scroll_to_row.take() {
                        table = table.scroll_to_row(row_nr, None);
                    }

                    table
                        .header(20.0, |mut header| {
                            // Row Index column
                            header.col(|ui| {
                                ui.strong("Row");
                            });
                            for col in &t.headers {
                                header.col(|ui| {
                                    ui.strong(col);
                                });
                            }
                        })
                        .body(|body| {
                            let row_height = text_height * 1.2;
                            // flat groups into a vec, append an empty row after every group
                            let indices = indices.iter().flatten().collect::<Vec<&usize>>();
                            body.rows(row_height, indices.len(), |idx, mut row| {
                                row.col(|ui| {
                                    ui.label(indices[idx].to_string());
                                });
                                for col in &t.rows[*indices[idx]] {
                                    row.col(|ui| {
                                        ui.label(col);
                                    });
                                }
                            });
                        });

                    ui.separator();

                    // Show stats
                    // How many groups
                    ui.label(format!("Groups: {}", indices.len()));
                    if ui.button("Export").clicked() {
                        let output = FileDialog::new().add_filter("csv", &["csv"]).save_file();
                        match output {
                            Some(f) => match write_table(&f, t, indices) {
                                Ok(_) => self.logs.push(LogMessage::new(
                                    format!("Exported to {:?}", f),
                                    LogLevel::Info,
                                )),
                                Err(e) => self.logs.push(LogMessage::new(
                                    format!("Failed to export to {:?}: {:?}", f, e),
                                    LogLevel::Error,
                                )),
                            },
                            None => self.logs.push(LogMessage::new(
                                String::from("Failed to select output"),
                                LogLevel::Warning,
                            )),
                        }
                    }
                });
            }
        }
    }
}

fn read_table(csv: PathBuf) -> Result<Table, std::io::Error> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&csv)?;
    let headers: Vec<String> = rdr.headers()?.into_iter().map(|b| b.to_owned()).collect();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for record in rdr.records() {
        if let Err(e) = record {
            return Err(e.into());
        }
        let row: Vec<String> = record.unwrap().into_iter().map(|b| b.to_owned()).collect();
        rows.push(row);
    }
    return Ok(Table {
        headers,
        rows,
        file: csv,
    });
}

// fn group_by_edit_distance(keys: &Vec<String>, max_step: usize) -> Vec<Vec<usize>> {
//     let mut groups: Vec<Vec<usize>> = (0..keys.len()).map(|i| vec![i]).collect();
//     for group in groups.iter_mut() {
//         for i in 0..keys.len() {
//             if group.contains(&i) {
//                 continue;
//             }
//             if levenshtein_distance(&keys[group[0]], &keys[i]) <= max_step {
//                 group.push(i);
//             }
//         }
//     }
//     return groups;
// }

fn cal_similarity(left: &str, right: &str) -> usize {
    let lev_dis = levenshtein_distance(left, right);
    let max_len = std::cmp::max(left.len(), right.len());

    // Meaning that both strings are empty
    if max_len == 0 {
        return 100;
    }
    return (max_len - lev_dis) * 100 / max_len;
}

fn cal_similarity_case_insentive(left: &str, right: &str) -> usize {
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    return cal_similarity(&left, &right);
}

#[allow(dead_code)]
fn group_by_similarity(
    keys: &Vec<String>,
    similarity: usize,
    case_sensitive: bool,
) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = (0..keys.len()).map(|i| vec![i]).collect();
    for group in groups.iter_mut() {
        for i in 0..keys.len() {
            if group.contains(&i) {
                continue;
            }
            let cal = if case_sensitive {
                cal_similarity
            } else {
                cal_similarity_case_insentive
            };

            if cal(&keys[group[0]], &keys[i]) >= similarity {
                group.push(i);
            }
        }
    }
    return groups;
}

fn group_by_similarity_v2(
    keys: &Vec<String>,
    similarity: usize,
    case_sensitive: bool,
) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = (0..keys.len()).map(|i| vec![i]).collect();
    let mut visited: Vec<bool> = vec![false; keys.len()];
    for group in groups.iter_mut() {
        for i in 0..keys.len() {
            if group.contains(&i) || visited[i] {
                continue;
            }
            let cal = if case_sensitive {
                cal_similarity
            } else {
                cal_similarity_case_insentive
            };

            if cal(&keys[group[0]], &keys[i]) >= similarity {
                group.push(i);
                visited[i] = true;
            }
        }
    }
    return groups;
}

fn write_table(
    csv: &PathBuf,
    table: &Table,
    groups: &Vec<Vec<usize>>,
) -> Result<(), std::io::Error> {
    let mut wtr = csv::WriterBuilder::new().has_headers(true).from_path(csv)?;
    // Add index header to original headers
    let headers: Vec<String> = vec!["Index".to_string()]
        .into_iter()
        .chain(table.headers.iter().cloned())
        .collect();
    let cols = headers.len();
    wtr.write_record(headers)?;
    for group in groups {
        for r_idx in group {
            let mut row = vec![r_idx.to_string()];
            row.extend(table.rows[*r_idx].iter().cloned());
            wtr.write_record(row)?;
        }
        // Write a empty row
        wtr.write_record([""].repeat(cols))?;
    }
    wtr.flush()?;
    return Ok(());
}

fn setup_custom_fonts(ctx: &egui::Context) {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = egui::FontDefinitions::default();

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "my_font".to_owned(),
        egui::FontData::from_static(include_bytes!("../fonts/NotoSansSC-Regular.otf")),
    );

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "my_font".to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("my_font".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}
