use crate::ui::{charts::stats, tabs::Tab};
use egui::{Color32, Frame, RichText, Ui, WidgetText};

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

        let is_dark = ui.visuals().dark_mode;

        // Cores adaptadas ao tema: mais vibrantes no escuro, mais sóbrias no claro
        let color_purple = if is_dark {
            egui::Color32::from_rgb(167, 105, 220)
        } else {
            egui::Color32::from_rgb(106, 27, 154)
        };
        let color_blue = if is_dark {
            egui::Color32::from_rgb(79, 159, 255)
        } else {
            egui::Color32::from_rgb(26, 115, 232)
        };
        let color_green = if is_dark {
            egui::Color32::from_rgb(52, 199, 89)
        } else {
            egui::Color32::from_rgb(19, 115, 51)
        };
        let color_orange = if is_dark {
            egui::Color32::from_rgb(255, 159, 64)
        } else {
            egui::Color32::from_rgb(176, 96, 0)
        };

        ui.columns(4, |cols| {
            draw_kpi_card(
                &mut cols[0],
                "TOTAL CADASTROS",
                &format!("{}", total_funds),
                color_purple,
            );
            draw_kpi_card(
                &mut cols[1],
                "RENDA FIXA",
                &format!("{}", renda_fixa),
                color_blue,
            );
            draw_kpi_card(&mut cols[2], "AÇÕES", &format!("{}", acoes), color_green);
            draw_kpi_card(
                &mut cols[3],
                "MULTIMERCADO",
                &format!("{}", multimercado),
                color_orange,
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
        // Extrai dados de ranking ordenados
        let sit_ranking = extract_ranking(&self.by_situation, "SIT", "TP_FUNDO");
        let class_ranking = extract_ranking(&self.by_class, "CLASSE", "TP_FUNDO");

        let avail_h = ui.available_height();
        let kpi_h = 80.0;
        let margin = 56.0;
        let charts_h = (avail_h - kpi_h - margin).max(380.0);
        let top_h = (charts_h * 0.56).max(200.0);
        let bot_h = (charts_h * 0.44).max(170.0);

        Frame::NONE
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                // ── Linha 1: KPI Cards ────────────────────────────────────────
                self.render_kpi_cards(ui);
                ui.add_space(12.0);

                // ── Linha 2: Top Situações  |  Top Classes ────────────
                ui.columns(2, |cols| {
                    render_ranking_card(
                        &mut cols[0],
                        "Distribuição por Situação",
                        &sit_ranking,
                        top_h,
                    );
                    render_ranking_card(
                        &mut cols[1],
                        "Distribuição por Classe de Fundo",
                        &class_ranking,
                        top_h,
                    );
                });

                ui.add_space(8.0);

                // ── Linha 3: Fundos por Ano (largura total) ────────────
                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.set_min_height(bot_h);
                    ui.label(
                        RichText::new("Fundos Cadastrados por Ano")
                            .size(13.0)
                            .strong(),
                    );
                    ui.separator();
                    ui.add_space(4.0);
                    stats::by_year_bar(&self.by_year, ui, bot_h - 30.0);
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
                    if class_str
                        .to_lowercase()
                        .contains(&class_name.to_lowercase())
                    {
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

fn draw_kpi_card(ui: &mut egui::Ui, title: &str, value: &str, accent_color: egui::Color32) {
    egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_height(58.0);
            ui.horizontal(|ui| {
                // Barra vertical colorida
                let (rect, _) = ui.allocate_exact_size(egui::vec2(4.0, 40.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(2), accent_color);

                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new(title).size(10.0).strong().color(
                        if ui.visuals().dark_mode {
                            Color32::from_rgb(140, 150, 165)
                        } else {
                            Color32::from_rgb(100, 110, 130)
                        },
                    ));
                    ui.add_space(2.0);
                    ui.label(RichText::new(value).size(22.0).strong().color(
                        if ui.visuals().dark_mode {
                            Color32::WHITE
                        } else {
                            Color32::from_rgb(15, 20, 35)
                        },
                    ));
                });
            });
        });
}

// ── Ranking helpers ───────────────────────────────────────────────────────

const RANK_COLORS: [Color32; 8] = [
    Color32::from_rgb(79, 159, 255),  // azul
    Color32::from_rgb(52, 199, 89),   // verde
    Color32::from_rgb(255, 159, 64),  // laranja
    Color32::from_rgb(167, 105, 220), // roxo
    Color32::from_rgb(255, 89, 94),   // vermelho
    Color32::from_rgb(0, 188, 212),   // ciano
    Color32::from_rgb(255, 202, 40),  // âmbar
    Color32::from_rgb(156, 39, 176),  // rosa
];

/// Extrai pares (categoria, valor) do DataFrame, ordenados do maior para o menor.
fn extract_ranking(df: &DataFrame, category_col: &str, value_col: &str) -> Vec<(String, u32)> {
    let mut items: Vec<(String, u32)> = match (df.column(category_col), df.column(value_col)) {
        (Ok(cats), Ok(vals)) => {
            let cats = cats
                .utf8()
                .expect("Failed to convert category column to utf8");
            let vals = vals.u32().expect("Failed to convert value column to u32");
            cats.into_no_null_iter()
                .zip(vals.into_no_null_iter())
                .filter(|(c, _)| !c.is_empty())
                .map(|(c, v)| (c.to_string(), v))
                .collect()
        }
        _ => return vec![],
    };
    items.sort_by_key(|b| std::cmp::Reverse(b.1));
    items
}

/// Renderiza um card com ranking de itens usando progress bars.
fn render_ranking_card(ui: &mut Ui, title: &str, items: &[(String, u32)], max_height: f32) {
    let max_val = items.first().map(|(_, v)| *v).unwrap_or(1).max(1) as f32;

    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(13.0).strong());
            ui.separator();
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(max_height - 42.0)
                .show(ui, |ui| {
                    for (i, (label, value)) in items.iter().enumerate() {
                        let pct = *value as f32 / max_val;
                        let color = RANK_COLORS[i % RANK_COLORS.len()];
                        let total = items.iter().map(|(_, v)| *v).sum::<u32>() as f32;
                        let real_pct = if total > 0.0 {
                            *value as f32 / total * 100.0
                        } else {
                            0.0
                        };

                        // Linha: posição + label + valor
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:2}.", i + 1))
                                    .size(11.0)
                                    .color(Color32::from_gray(150)),
                            );
                            ui.label(RichText::new(label.as_str()).size(11.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!("{}  {:.1}%", value, real_pct))
                                            .size(11.0),
                                    );
                                },
                            );
                        });

                        // Progress bar
                        ui.add(egui::ProgressBar::new(pct).desired_height(6.0).fill(color));

                        ui.add_space(2.0);
                    }
                });
        });
}
