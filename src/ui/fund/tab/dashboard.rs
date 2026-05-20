use crate::ui::{charts::stats, tabs::Tab};
use egui::{Frame, Ui, WidgetText};
use egui_extras::{Size, StripBuilder};
use polars::frame::DataFrame;

pub struct DashboardTab {
    pub title: String,
    pub by_year: DataFrame,
    pub by_situation: DataFrame,
    pub by_class: DataFrame,
}

impl DashboardTab {
    pub fn set_dataframes(
        &mut self,
        by_year: DataFrame,
        by_situation: DataFrame,
        by_class: DataFrame,
    ) {
        self.by_year = by_year;
        self.by_situation = by_situation;
        self.by_class = by_class;
    }

    fn render_kpi_cards(&self, ui: &mut Ui) {
        let total_funds = get_total_funds(&self.by_situation);
        let renda_fixa = get_class_funds(&self.by_class, "Renda Fixa");
        let acoes = get_class_funds(&self.by_class, "Ações");
        let multimercado = get_class_funds(&self.by_class, "Multimercado");

        ui.columns(4, |cols| {
            // Card 1: Total Geral
            draw_kpi_card(
                &mut cols[0],
                "TOTAL CADASTROS",
                &format!("{}", total_funds),
                egui::Color32::from_rgb(106, 27, 154), // Roxo
            );

            // Card 2: Renda Fixa
            draw_kpi_card(
                &mut cols[1],
                "RENDA FIXA",
                &format!("{}", renda_fixa),
                egui::Color32::from_rgb(26, 115, 232), // Azul
            );

            // Card 3: Ações
            draw_kpi_card(
                &mut cols[2],
                "AÇÕES",
                &format!("{}", acoes),
                egui::Color32::from_rgb(19, 115, 51), // Verde
            );

            // Card 4: Multimercado
            draw_kpi_card(
                &mut cols[3],
                "MULTIMERCADO",
                &format!("{}", multimercado),
                egui::Color32::from_rgb(176, 96, 0), // Laranja
            );
        });
    }
}

impl Tab for DashboardTab {
    fn title(&self) -> WidgetText {
        self.title.clone().into()
    }

    fn closeable(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        Frame::NONE.inner_margin(10.0).show(ui, |ui| {
            StripBuilder::new(ui)
                .size(Size::exact(62.0))
                .size(Size::relative(0.38)) // proporção para os primeiros gráficos
                .size(Size::remainder())    // restante para o gráfico de classes
                .vertical(|mut strip| {
                    // Linha 1: Cards de KPIs
                    strip.cell(|ui| {
                        self.render_kpi_cards(ui);
                    });

                    // Linha 2: Gráficos de Ano e Situação
                    strip.strip(|builder| {
                        builder
                            .sizes(Size::remainder().at_least(50.0), 2)
                            .horizontal(|mut strip| {
                                strip.cell(|ui| {
                                    ui.group(|ui| {
                                        ui.heading(
                                            egui::RichText::new("Quantidade x Ano").size(11.0),
                                        );
                                        ui.separator();
                                        stats::by_year_bar(&self.by_year, ui);
                                    });
                                });

                                strip.cell(|ui| {
                                    ui.group(|ui| {
                                        ui.heading(
                                            egui::RichText::new("Quantidade x Situação").size(11.0),
                                        );

                                        ui.separator();
                                        stats::by_category_bar(
                                            &self.by_situation,
                                            "SIT",
                                            "TP_FUNDO",
                                            "Situação",
                                            ui,
                                        );
                                    });
                                });
                            });
                    });

                    // Linha 3: Gráfico de Classes
                    strip.cell(|ui| {
                        ui.add_space(5.0);
                        ui.group(|ui| {
                            ui.heading(egui::RichText::new("Quantidade x Classe").size(11.0));
                            ui.separator();
                            stats::by_category_bar(
                                &self.by_class,
                                "CLASSE",
                                "TP_FUNDO",
                                "Classe",
                                ui,
                            );
                        });
                    });
                });
        });
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
    }
}

fn get_total_funds(df: &DataFrame) -> u32 {
    if let Ok(col) = df.column("TP_FUNDO") {
        let mut sum = 0;
        for i in 0..col.len() {
            if let Ok(val) = col.get(i) {
                sum += val.try_extract::<u32>().unwrap_or(0);
            }
        }
        sum
    } else {
        0
    }
}

fn get_class_funds(df: &DataFrame, class_name: &str) -> u32 {
    if let (Ok(class_col), Ok(count_col)) = (df.column("CLASSE"), df.column("TP_FUNDO")) {
        let mut total = 0;
        for i in 0..df.height() {
            if let (Ok(class_val), Ok(count_val)) = (class_col.get(i), count_col.get(i)) {
                if let Some(class_str) = class_val.get_str() {
                    if class_str.to_lowercase().contains(&class_name.to_lowercase()) {
                        total += count_val.try_extract::<u32>().unwrap_or(0);
                    }
                }
            }
        }
        total
    } else {
        0
    }
}

fn draw_kpi_card(
    ui: &mut egui::Ui,
    title: &str,
    value: &str,
    accent_color: egui::Color32,
) {
    egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_height(44.0);
            ui.horizontal(|ui| {
                // Barra vertical colorida
                let (rect, _response) = ui.allocate_exact_size(egui::vec2(4.0, 34.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, egui::CornerRadius::same(2), accent_color);
                
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).size(9.0).weak().strong());
                    ui.label(
                        egui::RichText::new(value)
                            .size(16.0)
                            .strong()
                            .color(if ui.visuals().dark_mode {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::BLACK
                            }),
                    );
                });
            });
        });
}
