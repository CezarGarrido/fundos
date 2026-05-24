use std::sync::{Arc, Mutex};

use crate::ui::design::Scale;
use crate::ui::{charts::stats, tabs::Tab};
use crate::util;
use egui::{Color32, Frame, RichText, Ui, WidgetText};
use egui_toast::{Toast, ToastKind, ToastOptions};
use fundos_common::types::{ClassStat, DashboardStats};

#[derive(Clone)]
pub struct DashboardTab {
    pub title: String,
    pub stats: Option<DashboardStats>,
    loading: bool,
    pending_stats: Option<Arc<Mutex<Option<DashboardStats>>>>,
    repaint_ctx: Option<egui::Context>,
}

impl DashboardTab {
    pub fn new(title: String) -> Self {
        Self {
            title,
            stats: None,
            loading: false,
            pending_stats: None,
            repaint_ctx: None,
        }
    }

    fn poll_pending(&mut self) {
        if let Some(pending) = self.pending_stats.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(stats) = data {
                let total = stats.total_funds;
                self.stats = Some(stats);
                self.loading = false;
                crate::util::toaster().add(Toast {
                    kind: ToastKind::Success,
                    text: format!("Dashboard carregado: {} fundos", total).into(),
                    options: ToastOptions::default().duration_in_seconds(3.0),
                    style: Default::default(),
                });
            } else {
                self.pending_stats = Some(pending);
            }
        }
    }

    fn trigger_load(&mut self) {
        if self.loading || self.stats.is_some() {
            return;
        }
        self.loading = true;

        let result: Arc<Mutex<Option<DashboardStats>>> = Arc::new(Mutex::new(None));
        let r = result.clone();
        self.pending_stats = Some(result);
        let ctx = self.repaint_ctx.clone();

        util::spawn_future(async move {
            match crate::api_client::get_dashboard_stats().await {
                Ok(stats) => *r.lock().unwrap() = Some(stats),
                Err(e) => log::error!("Dashboard stats error: {}", e),
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    fn render_kpi_cards(&self, ui: &mut Ui) {
        let total_funds = self
            .stats
            .as_ref()
            .map(|s| s.by_situation.iter().map(|x| x.count).sum::<usize>())
            .unwrap_or(0);
        let renda_fixa = get_class_funds(
            self.stats.as_ref().map(|s| &s.by_class[..]).unwrap_or(&[]),
            "Renda Fixa",
        );
        let acoes = get_class_funds(
            self.stats.as_ref().map(|s| &s.by_class[..]).unwrap_or(&[]),
            "Ações",
        );
        let multimercado = get_class_funds(
            self.stats.as_ref().map(|s| &s.by_class[..]).unwrap_or(&[]),
            "Multimercado",
        );

        let is_dark = ui.visuals().dark_mode;

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
        self.repaint_ctx = Some(ui.ctx().clone());
        self.poll_pending();
        self.trigger_load();

        if self.loading && self.stats.is_none() {
            crate::ui::loading::show(ui, "Carregando estatísticas...");
            return;
        }

        let default_stats = DashboardStats {
            total_funds: 0,
            by_year: vec![],
            by_class: vec![],
            by_situation: vec![],
        };
        let stats = self.stats.as_ref().unwrap_or(&default_stats);

        let sit_ranking: Vec<(String, usize)> = stats
            .by_situation
            .iter()
            .map(|s| (s.situation.clone(), s.count))
            .collect();
        let class_ranking: Vec<(String, usize)> = stats
            .by_class
            .iter()
            .filter(|c| !c.class.is_empty())
            .map(|c| (c.class.clone(), c.count))
            .collect();

        let avail_h = ui.available_height();
        let kpi_h = 80.0;
        let margin = 56.0;
        let charts_h = (avail_h - kpi_h - margin).max(380.0);
        let top_h = (charts_h * 0.56).max(200.0);
        let bot_h = (charts_h * 0.44).max(170.0);

        Frame::NONE
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                self.render_kpi_cards(ui);
                ui.add_space(12.0);

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

                ui.group(|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.set_min_height(bot_h);
                    ui.label(
                        RichText::new("Fundos Cadastrados por Ano")
                            .size(Scale::DEFAULT.button())
                            .strong(),
                    );
                    ui.separator();
                    ui.add_space(4.0);
                    stats::by_year_bar(&stats.by_year, ui, bot_h - 30.0);
                });
            });
        ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
    }
}

fn get_class_funds(classes: &[ClassStat], class_name: &str) -> usize {
    let cn = class_name.to_lowercase();
    classes
        .iter()
        .filter(|c| c.class.to_lowercase().contains(&cn))
        .map(|c| c.count)
        .sum()
}

fn draw_kpi_card(ui: &mut egui::Ui, title: &str, value: &str, accent_color: egui::Color32) {
    egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_height(58.0);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(4.0, 40.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(2), accent_color);

                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(title)
                            .size(Scale::DEFAULT.badge())
                            .strong()
                            .color(if ui.visuals().dark_mode {
                                Color32::from_rgb(140, 150, 165)
                            } else {
                                Color32::from_rgb(100, 110, 130)
                            }),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(value)
                            .size(Scale::DEFAULT.metric_large())
                            .strong()
                            .color(if ui.visuals().dark_mode {
                                Color32::WHITE
                            } else {
                                Color32::from_rgb(15, 20, 35)
                            }),
                    );
                });
            });
        });
}

const RANK_COLORS: [Color32; 8] = [
    Color32::from_rgb(79, 159, 255),
    Color32::from_rgb(52, 199, 89),
    Color32::from_rgb(255, 159, 64),
    Color32::from_rgb(167, 105, 220),
    Color32::from_rgb(255, 89, 94),
    Color32::from_rgb(0, 188, 212),
    Color32::from_rgb(255, 202, 40),
    Color32::from_rgb(156, 39, 176),
];

fn render_ranking_card(ui: &mut Ui, title: &str, items: &[(String, usize)], max_height: f32) {
    let max_val = items
        .first()
        .map(|(_, v)| *v)
        .unwrap_or(1)
        .max(1) as f32;

    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).size(Scale::DEFAULT.button()).strong());
            ui.separator();
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(max_height - 42.0)
                .show(ui, |ui| {
                    for (i, (label, value)) in items.iter().enumerate() {
                        let pct = *value as f32 / max_val;
                        let color = RANK_COLORS[i % RANK_COLORS.len()];
                        let total = items.iter().map(|(_, v)| *v).sum::<usize>() as f32;
                        let real_pct = if total > 0.0 {
                            *value as f32 / total * 100.0
                        } else {
                            0.0
                        };

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:2}.", i + 1))
                                    .size(Scale::DEFAULT.label())
                                    .color(Color32::from_gray(150)),
                            );
                            ui.label(RichText::new(label.as_str()).size(Scale::DEFAULT.label()));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{}  {:.1}%",
                                            value, real_pct
                                        ))
                                        .size(Scale::DEFAULT.label()),
                                    );
                                },
                            );
                        });

                        ui.add(egui::ProgressBar::new(pct).desired_height(6.0).fill(color));

                        ui.add_space(2.0);
                    }
                });
        });
}
