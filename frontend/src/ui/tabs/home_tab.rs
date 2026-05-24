use super::Tab;
use crate::ui::design::Scale;
use egui::{Ui, WidgetText};

#[derive(Clone, Debug)]
pub enum HomeAction {
    OpenSearch,
    OpenDashboard,
    OpenAtivos,
}

#[derive(Clone)]
pub struct HomeTab {
    pub title: String,
    pub action: Option<HomeAction>,
}

impl HomeTab {
    pub fn new(title: String) -> Self {
        HomeTab { title, action: None }
    }

    pub fn take_action(&mut self) -> Option<HomeAction> {
        self.action.take()
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
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);

                let is_dark = ui.visuals().dark_mode;
                let brand_color = if is_dark {
                    egui::Color32::from_rgb(180, 186, 196)
                } else {
                    egui::Color32::from_rgb(74, 85, 104)
                };

                ui.label(
                    egui::RichText::new(egui_phosphor::regular::CHART_LINE_UP.to_string())
                        .size(Scale::ICON_BRAND)
                        .color(brand_color),
                );

                ui.add_space(12.0);

                ui.label(
                    egui::RichText::new("Fundos")
                        .size(Scale::DEFAULT.heading_1())
                        .strong()
                        .color(if is_dark {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgb(44, 55, 72)
                        }),
                );

                ui.add_space(40.0);

                if draw_menu_item(ui, "Pesquisar Fundos", &["Ctrl", "F"]) {
                    self.action = Some(HomeAction::OpenSearch);
                }
                ui.add_space(6.0);
                if draw_menu_item(ui, "Painel Geral", &["Ctrl", "D"]) {
                    self.action = Some(HomeAction::OpenDashboard);
                }
                ui.add_space(6.0);
                if draw_menu_item(ui, "Ativos do Mercado", &["Ctrl", "A"]) {
                    self.action = Some(HomeAction::OpenAtivos);
                }
            });
        });
    }
}

fn draw_menu_item(ui: &mut egui::Ui, label: &str, shortcut_keys: &[&str]) -> bool {
    let row_width = 320.0;
    let row_height = 32.0;
    let is_dark = ui.visuals().dark_mode;

    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(row_width, row_height), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.hovered() {
        let hover_bg = if is_dark {
            egui::Color32::from_rgb(33, 35, 41)
        } else {
            egui::Color32::from_rgb(240, 244, 255)
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), hover_bg);
    }

    ui.scope_builder(
        egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 4.0))),
        |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(label)
                        .size(Scale::DEFAULT.button())
                        .color(if response.hovered() {
                            if is_dark {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_rgb(26, 115, 232)
                            }
                        } else if is_dark {
                            egui::Color32::from_rgb(200, 200, 200)
                        } else {
                            egui::Color32::from_rgb(74, 85, 104)
                        }),
                );

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
                            if is_dark {
                                egui::Color32::from_rgb(180, 180, 180)
                            } else {
                                egui::Color32::from_rgb(100, 100, 100)
                            },
                        );

                        if i < shortcut_keys.len() - 1 {
                            ui.label(
                                egui::RichText::new("+").size(Scale::DEFAULT.badge()).weak(),
                            );
                        }
                    }
                });
            });
        },
    );

    response.clicked()
}
