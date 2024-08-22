use std::collections::BTreeSet;

use crate::config::{get_string, load_as_string, save_code};

use super::Tab;
use egui::{Align2, TopBottomPanel, Ui, WidgetText};
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};
use egui_toast::{Toast, ToastOptions, Toasts};

pub struct ConfigTab {
    pub title: String,
    code: String,
    toasts: egui_toast::Toasts,
}

impl ConfigTab {
    pub fn new(title: String) -> Self {
        let mut toasts = Toasts::new()
            .anchor(Align2::RIGHT_BOTTOM, (-10.0, -10.0)) // 10 units from the bottom right corner
            .direction(egui::Direction::BottomUp);

        ConfigTab {
            toasts,
            title,
            code: load_as_string(),
        }
    }
}

impl Tab for ConfigTab {
    fn title(&self) -> WidgetText {
        self.title.clone().into()
    }

    fn closeable(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            let button_color = ui.visuals().selection.bg_fill;

            let button = egui::widgets::Button::new("Salvar")
                .fill(button_color)
                .min_size(egui::vec2(150.0, 40.0)); // Ajuste o tamanho conforme necessário

            if ui.add(button).clicked() {
                if let Err(e) = save_code(self.code.clone()) {
                    eprintln!("Erro ao salvar código: {}", e);

                    self.toasts.add(Toast {
                        kind: egui_toast::ToastKind::Error,
                        text: "Erro ao salvar".into(),
                        options: ToastOptions::default().duration_in_seconds(2.0),
                    });
                } else {
                    eprintln!("Código salvo com sucesso.");
                    self.toasts.add(Toast {
                        kind: egui_toast::ToastKind::Info,
                        text: "Código salvo com sucesso".into(),
                        options: ToastOptions::default().duration_in_seconds(2.0),
                    });
                }
            }
        });
        ui.add_space(5.0);

        CodeEditor::default()
            .id_source("config_editor")
            .with_rows(12)
            .with_fontsize(14.0)
            .with_theme(ColorTheme::SONOKAI)
            .with_syntax(toml())
            .with_numlines(true)
            .show(ui, &mut self.code);

        self.toasts.show(ui.ctx())
    }
}
pub fn toml() -> Syntax {
    Syntax {
        language: "TOML",
        case_sensitive: true,
        comment: "#",
        comment_multiline: ["#", "#"],
        hyperlinks: BTreeSet::from([]),
        keywords: BTreeSet::from([
            "path",
            "description",
            "=",
            "start_date",
            "end_date",
            "url",
            "",
        ]),
        types: BTreeSet::from([]),
        special: BTreeSet::from(["false", "true", "{start_date}", "{end_date}"]),
    }
}
