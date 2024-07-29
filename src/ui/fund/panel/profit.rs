use chrono::NaiveDate;
use egui::{Align2, Frame, Layout, Vec2, Widget};
use egui_extras::DatePickerButton;
use polars::frame::DataFrame;
use tokio::sync::mpsc::UnboundedSender;

use crate::{message, ui::charts};

#[derive(Debug, PartialEq)]
pub enum FilterMonth {
    SixMonth,
    TwelveMonth,
    TwentyFourMonth,
    Custom,
}

pub struct ProfitUI {
    pub profit_filter_date: FilterMonth,
    pub profit_filter_start_date: NaiveDate,
    pub profit_filter_end_date: NaiveDate,
    pub open_profit_filter: bool,
    pub profit: DataFrame,
    pub cdi: DataFrame,
    pub ibov: DataFrame,
    pub cnpj: String,
    pub loading: bool,
    pub sender: Option<UnboundedSender<message::Message>>,
}

impl Default for ProfitUI {
    fn default() -> Self {
        let now = chrono::offset::Utc::now().date_naive();
        let start_date = now
            .checked_sub_signed(chrono::Duration::days(6 * 30))
            .unwrap();

        ProfitUI {
            cnpj: String::from(""),
            sender: None,
            profit: DataFrame::empty(),
            cdi: DataFrame::empty(),
            profit_filter_date: FilterMonth::SixMonth,
            profit_filter_start_date: start_date,
            profit_filter_end_date: now,
            open_profit_filter: false,
            loading: false,
            ibov: DataFrame::empty(),
        }
    }
}

impl ProfitUI {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label(format!(
                                "{} Gráfico de Rentabilidade",
                                egui_phosphor::regular::CHART_LINE
                            ));
                        });
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        let cnpj = self.cnpj.to_string();
                        ui.horizontal(|ui| {
                            self.create_filter_buttons(ui, &cnpj);
                            self.show_profit_filter_window(ui, &cnpj);
                        });
                    });
                });
            });
            ui.separator();
            egui::TopBottomPanel::top("vl_pl").show_inside(ui, |ui| {
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.weak("Rentabilidade");

                    self.profit
                        .column("RENT_ACUM")
                        .ok()
                        .and_then(|col| col.get(self.profit.height() - 1).ok())
                        .and_then(|val| val.to_string().into())
                        .and_then(|value_str| value_str.parse::<f64>().ok())
                        .map(|v| ui.heading(format!("%{}", v)))
                        .unwrap_or_else(|| ui.label("-"));
                });
                ui.add_space(8.0);
            });
            Frame::none().inner_margin(10.0).show(ui, |ui| {
                charts::fund::profit_chart(&self.profit, &self.cdi, &self.ibov, ui);
                // Ajuste conforme o tipo real dos seus dados
            });
        });
    }

    fn create_filter_buttons(&mut self, ui: &mut egui::Ui, cnpj: &str) {
        ui.add_enabled_ui(!self.loading, |ui| {
            if ui
                .selectable_value(
                    &mut self.profit_filter_date,
                    FilterMonth::Custom,
                    egui_phosphor::regular::CALENDAR.to_string(),
                )
                .clicked()
            {
                self.open_profit_filter = true;
            }
        });

        self.create_filter_button(ui, FilterMonth::TwentyFourMonth, "2A", cnpj);
        self.create_filter_button(ui, FilterMonth::TwelveMonth, "1A", cnpj);
        self.create_filter_button(ui, FilterMonth::SixMonth, "6M", cnpj);
    }

    fn create_filter_button(
        &mut self,
        ui: &mut egui::Ui,
        filter: FilterMonth,
        label: &str,
        cnpj: &str,
    ) {
        ui.add_enabled_ui(!self.loading, |ui| {
            if ui
                .selectable_value(&mut self.profit_filter_date, filter, label)
                .clicked()
            {
                let now = chrono::offset::Utc::now().date_naive();
                match self.profit_filter_date {
                    FilterMonth::SixMonth => {
                        let start_date = now
                            .checked_sub_signed(chrono::Duration::days(6 * 30))
                            .unwrap();
                        self.send_profit_message(cnpj, start_date, now);
                    }
                    FilterMonth::TwelveMonth => {
                        let start_date = now
                            .checked_sub_signed(chrono::Duration::days(12 * 30))
                            .unwrap();
                        self.send_profit_message(cnpj, start_date, now);
                    }
                    FilterMonth::TwentyFourMonth => {
                        let start_date = now
                            .checked_sub_signed(chrono::Duration::days(24 * 30))
                            .unwrap();
                        self.send_profit_message(cnpj, start_date, now);
                    }
                    FilterMonth::Custom => {}
                }

                self.loading = true;
            }
        });
    }

    fn send_profit_message(
        &mut self,
        cnpj: &str,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) {
        let _ = self.sender.clone().unwrap().send(message::Message::Profit(
            cnpj.to_string(),
            start_date,
            end_date,
        ));
        self.loading = true;
    }

    fn show_profit_filter_window(&mut self, ui: &mut egui::Ui, cnpj: &str) {
        let mut open_profit = self.open_profit_filter;
        let mut other = true;
        egui::Window::new("Período")
            .resizable(false)
            .collapsible(false)
            .default_width(200.0)
            .max_width(200.0)
            .max_height(350.0)
            .anchor(Align2::RIGHT_TOP, Vec2::new(0.0, 150.0))
            .open(&mut open_profit)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Data Inicial:");
                    DatePickerButton::new(&mut self.profit_filter_start_date)
                        .id_source("data_ini")
                        .ui(ui);
                });

                ui.horizontal(|ui| {
                    ui.label("Data Final:  ");
                    DatePickerButton::new(&mut self.profit_filter_end_date)
                        .id_source("data_fim")
                        .ui(ui);
                });

                ui.vertical_centered(|ui| {
                    if ui
                        .add_enabled(!self.loading, egui::Button::new("Aplicar"))
                        .clicked()
                    {
                        self.send_profit_message(
                            cnpj,
                            self.profit_filter_start_date,
                            self.profit_filter_end_date,
                        );
                        other = false;
                    }
                });
            });

        self.open_profit_filter = open_profit && other;
    }
}
