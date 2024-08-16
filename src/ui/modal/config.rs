use crate::config::{config_to_value, schema_to_value};
use crate::message::Message;
use eframe::egui;
use eframe::egui::Context;
use egui::{CentralPanel, SidePanel, TopBottomPanel, Ui, Widget};
use egui_extras::DatePickerButton;
use serde_yaml::Value;
use tokio::sync::mpsc;

#[derive(PartialEq, serde::Deserialize, serde::Serialize)]
pub enum Mode {
    Recommended,
    Other,
}

pub struct Config {
    pub value: Value,
    pub show: bool,
    pub initial: bool,
    pub current_selection: Value,
    pub current_schema: Value,
    pub mode: Mode,
    schema: Value,
}

impl Config {
    pub fn new(_sender: mpsc::UnboundedSender<Message>) -> Self {
        let value = config_to_value().unwrap_or_default();
        let schema = schema_to_value().unwrap_or_default();

        Self {
            initial: false,
            show: false,
            value,
            current_selection: Value::Null,
            current_schema: Value::Null,
            mode: Mode::Recommended,
            schema,
        }
    }

    pub fn open(&mut self, value: bool) {
        self.show = value;
    }

    fn initial_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.0);
        ui.label("Antes de iniciar, é preciso selecionar um período de tempo para que a aplicação possa baixar os arquivos necessários.");
        ui.add_space(5.0);
        ui.separator();

        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.mode, Mode::Recommended, "Padrão (Recomendado)");
            ui.add_space(5.0);
            ui.weak("Baixa os arquivos dos últimos 2 anos.");
        });

        ui.horizontal(|ui| {
            ui.radio_value(&mut self.mode, Mode::Other, "Personalizado");
            ui.add_space(5.0);
            ui.weak("Selecione o periodo de tempo.");
        });

        if self.mode == Mode::Other {
            ui.add_space(5.0);
            ui.separator();
            self.config_ui(ui);
        }

        TopBottomPanel::bottom("config_bottom")
            .exact_height(30.0)
            .show_inside(ui, |ui| {
                ui.add_space(5.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Salvar").clicked() {
                        // Save logic here
                    }
                });
            });
    }

    fn config_ui(&mut self, ui: &mut egui::Ui) {
        SidePanel::left("tree_side_panel")
            .width_range(150.0..=200.0)
            .show_inside(ui, |ui| {
                my_build_side_panel(
                    &self.schema,
                    ui,
                    "",
                    &mut self.current_selection,
                    &mut self.current_schema,
                    &self.value,
                )
            });

        CentralPanel::default().show_inside(ui, |ui| {
            build_ui_from_schema(&self.current_schema, ui, "", &mut self.current_selection);
        });
    }

    pub fn show(&mut self, ctx: &Context) {
        let mut open = self.show;
        egui::Window::new("Configuração")
            .resizable(false)
            .collapsible(false)
            .default_width(550.0)
            .max_width(550.0)
            .max_height(500.0)
            .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, 150.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.set_width(550.0);
                if self.initial {
                    self.initial_ui(ui);
                } else {
                    self.config_ui(ui);
                }
            });
    }
}
fn my_build_side_panel(
    schema: &Value,
    ui: &mut Ui,
    path: &str,
    current_selection: &mut Value,
    current_schema: &mut Value,
    all: &Value,
) {
    let title = schema
        .get(&Value::String("title".to_string()))
        .and_then(|v| v.as_str())
        .unwrap_or(path);

    // Verifica o tipo do schema
    if let Some(Value::String(type_str)) = schema.get(&Value::String("type".to_string())) {
        if type_str == "object" {
            let properties = schema.get(&Value::String("properties".to_string()));

            if let Some(Value::Mapping(properties)) = properties {
                // Verifica se todos os filhos são do tipo "object"
                let all_object_children = properties.iter().all(|(_, value)| {
                    if let Value::Mapping(field_schema) = value {
                        if let Some(Value::String(type_str)) =
                            field_schema.get(&Value::String("type".to_string()))
                        {
                            type_str == "object"
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                });

                if all_object_children {
                    // Se todos os filhos são "object", usa collapsing
                    ui.collapsing(title, |ui| {
                        for (key, value) in properties.iter() {
                            let new_path = format!("{}/{}", path, key.as_str().unwrap_or_default());
                            my_build_side_panel(
                                value,
                                ui,
                                &new_path,
                                current_selection,
                                current_schema,
                                all,
                            );
                        }
                    });
                } else {
                    // Se não houver todos os filhos com tipo "object", cria um botão
                    if ui.button(title).clicked() {
                        if let Some(selected_value) = get_value_from_path(all, path) {
                            *current_selection = selected_value.clone();
                            *current_schema = schema.clone();
                        }
                    }
                }
            }
        } else {
            // Caso o tipo não seja "object", apenas cria um botão
            if ui.button(title).clicked() {
                if let Some(selected_value) = get_value_from_path(all, path) {
                    *current_selection = selected_value.clone();
                    *current_schema = schema.clone();
                }
            }
        }
    } else {
        // Caso não tenha a chave "type", apenas cria um botão
        if ui.button(title).clicked() {
            if let Some(selected_value) = get_value_from_path(all, path) {
                *current_selection = selected_value.clone();
                *current_schema = schema.clone();
            }
        }
    }
}

fn get_value_from_path<'a>(all: &'a Value, path: &'a str) -> Option<&'a Value> {
    let keys: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = all;

    for key in keys {
        if let Value::Mapping(map) = current {
            if let Some(v) = map.get(&Value::String(key.to_string())) {
                current = v;
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
    Some(current)
}

fn build_ui_from_schema(schema: &Value, ui: &mut Ui, path: &str, current_selection: &mut Value) {
    let title = schema.get("title").and_then(|v| v.as_str()).unwrap_or(path);

    if let Some(Value::String(type_str)) = schema.get("type") {
        match type_str.as_str() {
            "date" => {
                if let Value::String(ref mut s) = get_mut_value(current_selection, path) {
                    if let Ok(mut date) = chrono::NaiveDate::parse_from_str(s, "%d/%m/%Y") {
                        ui.horizontal(|ui| {
                            ui.push_id(path, |ui| {
                                ui.label(title);
                                // Note: Replace DatePickerButton with your actual date picker implementation
                                DatePickerButton::new(&mut date)
                                    .id_source("datepicker_{}")
                                    .ui(ui)
                                    .clicked();

                                *s = date.format("%d/%m/%Y").to_string();
                            });
                        });
                    }
                };
            }
            "string" => {
                if let Value::String(ref mut s) = get_mut_value(current_selection, path) {
                    ui.horizontal(|ui| {
                        ui.label(title);
                        ui.text_edit_singleline(s);
                    });
                }
            }
            "number" => {
                if let Value::Number(ref mut v) = get_mut_value(current_selection, path) {
                    ui.horizontal(|ui| {
                        if v.is_i64() {
                            let mut value = v.as_i64().unwrap();
                            ui.label(title);
                            ui.add(egui::widgets::DragValue::new(&mut value));
                            *v = serde_yaml::Number::from(value);
                        } else if v.is_f64() {
                            let mut value = v.as_f64().unwrap();
                            ui.label(title);
                            ui.add(egui::widgets::DragValue::new(&mut value));
                            *v = serde_yaml::Number::from(value);
                        }
                    });
                }
            }
            "boolean" => {
                if let Value::Bool(ref mut b) = get_mut_value(current_selection, path) {
                    ui.horizontal(|ui| {
                        ui.label(title);
                        ui.checkbox(b, "");
                    });
                }
            }
            "object" => {
                if let Value::Mapping(ref mut map) = get_mut_value(current_selection, path) {
                    if let Some(properties) = schema.get("properties").and_then(|v| v.as_mapping())
                    {
                        ui.group(|ui| {
                            ui.label(title);
                            for (key, value) in properties {
                                let new_path =
                                    format!("{}/{}", path, key.as_str().unwrap_or_default());
                                // Insira o valor padrão se a chave não estiver presente
                                // Verifica o tipo do valor no esquema
                                let entry = map.entry(key.clone()).or_insert_with(|| {
                                    if let Some(value_type) =
                                        value.get("type").and_then(|v| v.as_str())
                                    {
                                        match value_type {
                                            "string" => Value::String(String::new()), // Valor padrão para string
                                            "number" => Value::Number(serde_yaml::Number::from(0)), // Valor padrão para número
                                            "object" => Value::Mapping(serde_yaml::Mapping::new()), // Valor padrão para objeto
                                            "array" => Value::Sequence(vec![]), // Valor padrão para array
                                            "boolean" => Value::Bool(false), // Valor padrão para booleano
                                            _ => Value::Null, // Default para tipos desconhecidos
                                        }
                                    } else {
                                        Value::Null // Default caso o tipo não seja especificado
                                    }
                                });
                                build_ui_from_schema(value, ui, &new_path, entry);
                            }
                        });
                    }
                }
            }
            "array" => {
                if let Value::Sequence(ref mut seq) = get_mut_value(current_selection, path) {
                    if let Some(item_schema) = schema.get("items") {
                        for (index, item) in seq.iter_mut().enumerate() {
                            let new_path = format!("{}/{}", path, index);
                            build_ui_from_schema(item_schema, ui, &new_path, item);
                        }
                    }
                }
            }
            _ => {
                ui.label(format!("Unknown type: {}", type_str));
            }
        }
    } else {
        ui.label("No type specified");
    }
}

fn get_mut_value<'a>(current_selection: &'a mut Value, path: &str) -> &'a mut Value {
    let keys: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = current_selection;

    for key in keys {
        match current {
            Value::Mapping(map) => {
                current = map.get_mut(&Value::String(key.to_string())).unwrap();
            }
            Value::Sequence(seq) => {
                let index = key.parse::<usize>().unwrap();
                current = &mut seq[index];
            }
            _ => println!("Invalid path {}", path),
        }
    }
    current
}
