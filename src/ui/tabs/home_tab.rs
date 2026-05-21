use super::Tab;
use crate::{history::History, message::Message};
use chrono::{Duration, Local, Months};
use egui::{CentralPanel, Ui, WidgetText};
use tokio::sync::mpsc;


pub struct HomeTab {
    pub title: String,
    pub history: History,
    pub sender: mpsc::UnboundedSender<Message>,
}

impl HomeTab {
    pub fn new(title: String, sender: mpsc::UnboundedSender<Message>, history: History) -> Self {
        HomeTab {
            title,
            sender,
            history,
        }
    }
}

impl Tab for HomeTab {
    fn title(&self) -> WidgetText {
        self.title.clone().into()
    }

    fn closeable(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut Ui) {
        CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                
                // 1. Logo simples e elegante (sem círculo, igual ao Antigravity)
                let is_dark = ui.visuals().dark_mode;
                let brand_color = if is_dark {
                    egui::Color32::from_rgb(180, 186, 196)
                } else {
                    egui::Color32::from_rgb(74, 85, 104)
                };
                
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::CHART_LINE_UP.to_string())
                        .size(52.0)
                        .color(brand_color),
                );
                
                ui.add_space(12.0);
                
                // 2. Título principal
                ui.label(
                    egui::RichText::new("Fundos")
                        .size(24.0)
                        .strong()
                        .color(if is_dark { egui::Color32::WHITE } else { egui::Color32::from_rgb(44, 55, 72) }),
                );
                
                ui.add_space(40.0);
                
                // 3. Menu de Ações no estilo Antigravity
                if draw_menu_item(ui, "Pesquisar Fundos", &["Ctrl", "F"]) {
                    let _ = self.sender.send(Message::OpenSearchWindow(true));
                }
                
                ui.add_space(6.0);
                
                if draw_menu_item(ui, "Painel Geral", &["Ctrl", "D"]) {
                    let _ = self.sender.send(Message::OpenDashboardTab);
                }
                
                ui.add_space(6.0);
                
                if draw_menu_item(ui, "Ativos do Mercado", &["Ctrl", "A"]) {
                    let end = Local::now().naive_local().date();
                    let start = end
                        .checked_sub_months(Months::new(6))
                        .unwrap_or(end - Duration::days(183));
                    let _ = self.sender.send(Message::OpenAtivosTab(start, end));
                }
                
                // 4. Seção Visto Recentemente
                let recenteds = self.history.get_most_accesseds();
                if !recenteds.is_empty() {
                    ui.add_space(30.0);
                    
                    // Separador sutil
                    let sep_color = if is_dark {
                        egui::Color32::from_rgb(45, 48, 56)
                    } else {
                        egui::Color32::from_rgb(226, 232, 240)
                    };
                    let (sep_rect, _) = ui.allocate_exact_size(egui::vec2(320.0, 1.0), egui::Sense::hover());
                    ui.painter().rect_filled(sep_rect, egui::CornerRadius::ZERO, sep_color);
                    
                    ui.add_space(15.0);
                    
                    ui.label(
                        egui::RichText::new("VISTO RECENTEMENTE")
                            .size(10.0)
                            .strong()
                            .color(if is_dark { egui::Color32::from_rgb(100, 100, 100) } else { egui::Color32::from_rgb(160, 160, 160) }),
                    );
                    ui.add_space(8.0);
                    
                    // Lista vertical de itens recentes
                    for (cnpj, name) in recenteds.iter().take(4) {
                        let short_name = if name.len() > 32 {
                            format!("{}...", &name[..29])
                        } else {
                            name.clone()
                        };
                        
                        if draw_recent_item(ui, &short_name, cnpj) {
                            let _ = self.sender.send(Message::NewTab(cnpj.clone()));
                        }
                        ui.add_space(4.0);
                    }
                }
            });
        });
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
    }
}

fn draw_menu_item(ui: &mut egui::Ui, label: &str, shortcut_keys: &[&str]) -> bool {
    let row_width = 320.0;
    let row_height = 32.0;
    let is_dark = ui.visuals().dark_mode;
    
    // Allocate space for the row
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(row_width, row_height),
        egui::Sense::click(),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    
    // Hover highlight
    if response.hovered() {
        let hover_bg = if is_dark {
            egui::Color32::from_rgb(33, 35, 41)
        } else {
            egui::Color32::from_rgb(240, 244, 255)
        };
        ui.painter().rect_filled(rect, egui::CornerRadius::same(6), hover_bg);
    }
    
    // Create a child UI for the row content using scope_builder
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 4.0))),
        |ui| {
            ui.horizontal(|ui| {
                // Left aligned: label
                ui.label(
                    egui::RichText::new(label)
                        .size(13.0)
                        .color(if response.hovered() {
                            if is_dark { egui::Color32::WHITE } else { egui::Color32::from_rgb(26, 115, 232) }
                        } else {
                            if is_dark { egui::Color32::from_rgb(200, 200, 200) } else { egui::Color32::from_rgb(74, 85, 104) }
                        }),
                );
                
                // Right aligned: shortcuts in reading order (iterated in reverse order inside right_to_left layout)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for (i, key) in shortcut_keys.iter().rev().enumerate() {
                        let kbd_bg = if is_dark {
                            egui::Color32::from_rgb(45, 48, 56)
                        } else {
                            egui::Color32::from_rgb(243, 244, 246)
                        };
                        
                        let kbd_border = if is_dark {
                            egui::Color32::from_rgb(60, 64, 72)
                        } else {
                            egui::Color32::from_rgb(209, 213, 219)
                        };
                        
                        let (kbd_rect, _) = ui.allocate_exact_size(
                            egui::vec2(key.len() as f32 * 6.0 + 12.0, 18.0),
                            egui::Sense::hover(),
                        );
                        
                        ui.painter().rect(
                            kbd_rect,
                            egui::CornerRadius::same(3),
                            kbd_bg,
                            egui::Stroke::new(1.0, kbd_border),
                            egui::StrokeKind::Inside,
                        );
                        
                        ui.painter().text(
                            kbd_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            key.to_string(),
                            egui::FontId::proportional(10.0),
                            if is_dark { egui::Color32::from_rgb(180, 180, 180) } else { egui::Color32::from_rgb(100, 100, 100) },
                        );

                        if i < shortcut_keys.len() - 1 {
                            ui.label(
                                egui::RichText::new("+")
                                    .size(10.0)
                                    .weak(),
                            );
                        }
                    }
                });
            });
        }
    );
    
    response.clicked()
}

fn draw_recent_item(ui: &mut egui::Ui, name: &str, cnpj: &str) -> bool {
    let row_width = 320.0;
    let row_height = 32.0;
    let is_dark = ui.visuals().dark_mode;
    
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(row_width, row_height),
        egui::Sense::click(),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    
    if response.hovered() {
        let hover_bg = if is_dark {
            egui::Color32::from_rgb(33, 35, 41)
        } else {
            egui::Color32::from_rgb(240, 244, 255)
        };
        ui.painter().rect_filled(rect, egui::CornerRadius::same(6), hover_bg);
    }
    
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 4.0))),
        |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(name)
                        .size(12.0)
                        .color(if response.hovered() {
                            if is_dark { egui::Color32::WHITE } else { egui::Color32::from_rgb(26, 115, 232) }
                        } else {
                            if is_dark { egui::Color32::from_rgb(180, 180, 180) } else { egui::Color32::from_rgb(74, 85, 104) }
                        }),
                ).on_hover_text(format!("CNPJ: {}", cnpj));
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::ARROW_SQUARE_UP_RIGHT.to_string())
                            .size(11.0)
                            .weak(),
                    );
                });
            });
        }
    );
    
    response.clicked()
}
