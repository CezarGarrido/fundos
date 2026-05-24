use crate::ui::{
    charts::{self, profit::Indice},
    design::{Components, Scale, Typography},
    loading,
};
use chrono::{Datelike, NaiveDate};
use egui::{Align2, Color32, Layout, Vec2, Widget};
use egui_extras::DatePickerButton;
use fundos_common::types::TimeSeriesPoint;
use jiff::civil::{date as jiff_date, Date as JiffDate};

#[derive(Clone, Debug, PartialEq)]
pub enum FilterMonth {
    OneMonth,
    SixMonth,
    TwelveMonth,
    TwentyFourMonth,
    Ytd,
    Custom,
}

#[derive(Clone)]
pub struct ProfitUI {
    pub profit_filter_date: FilterMonth,
    pub profit_filter_start_date: JiffDate,
    pub profit_filter_end_date: JiffDate,
    pub open_profit_filter: bool,
    pub fund_series: Vec<TimeSeriesPoint>,
    pub cdi_series: Vec<TimeSeriesPoint>,
    pub ibov_series: Vec<TimeSeriesPoint>,
    pub cnpj: String,
    pub loading: bool,
    pub loading_status: String,
    /// Set by the UI when user requests a new date range. Parent reads and clears.
    pub requested_range: Option<(NaiveDate, NaiveDate)>,
}

impl Default for ProfitUI {
    fn default() -> Self {
        let now = chrono::offset::Utc::now().date_naive();
        let start_date = now
            .checked_sub_signed(chrono::Duration::days(6 * 30))
            .unwrap();

        ProfitUI {
            cnpj: String::new(),
            fund_series: vec![],
            cdi_series: vec![],
            ibov_series: vec![],
            profit_filter_date: FilterMonth::SixMonth,
            profit_filter_start_date: jiff_date(
                start_date.year() as i16,
                start_date.month() as i8,
                start_date.day() as i8,
            ),
            profit_filter_end_date: jiff_date(
                now.year() as i16,
                now.month() as i8,
                now.day() as i8,
            ),
            open_profit_filter: false,
            loading: true,
            loading_status: "Carregando rentabilidade...".to_string(),
            requested_range: None,
        }
    }
}

impl ProfitUI {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let dark = ui.visuals().dark_mode;
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label(Typography::heading_3("Gráfico de Rentabilidade", dark));
                        });
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        let cnpj = self.cnpj.clone();
                        ui.horizontal(|ui| {
                            self.create_filter_buttons(ui);
                            self.show_profit_filter_window(ui);
                        });
                        _ = cnpj;
                    });
                });
            });
            ui.separator();
            ui.add_space(5.0);

            if self.loading {
                loading::show(ui, &self.loading_status);
            } else {
                ui.vertical(|ui| {
                    ui.weak("Rentabilidade");
                    if let Some(last) = self.fund_series.last() {
                        ui.heading(format!("{:.2}%", last.value));
                    } else {
                        ui.label("-");
                    }
                });
                ui.separator();

                ui.add_space(5.0);

                let cdi_color = Color32::from_rgb(230, 126, 34);
                let ibov_color = Color32::from_rgb(52, 152, 219);
                let dark = ui.visuals().dark_mode;

                Components::card(ui, dark, 5, |ui| {
                    charts::profit::chart(
                        &self.fund_series,
                        vec![
                            Indice {
                                name: "CDI".to_string(),
                                color: cdi_color,
                                series: self.cdi_series.clone(),
                            },
                            Indice {
                                name: "IBOV".to_string(),
                                color: ibov_color,
                                series: self.ibov_series.clone(),
                            },
                        ],
                        ui,
                    );
                });
            }
        });
    }

    fn create_filter_buttons(&mut self, ui: &mut egui::Ui) {
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

        self.create_filter_button(ui, FilterMonth::TwentyFourMonth, "2A");
        self.create_filter_button(ui, FilterMonth::TwelveMonth, "1A");
        self.create_filter_button(ui, FilterMonth::Ytd, "YTD");
        self.create_filter_button(ui, FilterMonth::SixMonth, "6M");
        self.create_filter_button(ui, FilterMonth::OneMonth, "1M");
    }

    fn create_filter_button(&mut self, ui: &mut egui::Ui, filter: FilterMonth, label: &str) {
        ui.add_enabled_ui(!self.loading, |ui| {
            if ui
                .selectable_value(&mut self.profit_filter_date, filter, label)
                .clicked()
            {
                let now = chrono::offset::Utc::now().date_naive();
                let (start, end) = match self.profit_filter_date {
                    FilterMonth::OneMonth => (
                        now.checked_sub_signed(chrono::Duration::days(30))
                            .unwrap(),
                        now,
                    ),
                    FilterMonth::SixMonth => (
                        now.checked_sub_signed(chrono::Duration::days(6 * 30))
                            .unwrap(),
                        now,
                    ),
                    FilterMonth::TwelveMonth => (
                        now.checked_sub_signed(chrono::Duration::days(12 * 30))
                            .unwrap(),
                        now,
                    ),
                    FilterMonth::TwentyFourMonth => (
                        now.checked_sub_signed(chrono::Duration::days(24 * 30))
                            .unwrap(),
                        now,
                    ),
                    FilterMonth::Ytd => (
                        chrono::NaiveDate::from_ymd_opt(now.year(), 1, 1).unwrap(),
                        now,
                    ),
                    FilterMonth::Custom => return,
                };

                self.requested_range = Some((start, end));
                self.loading = true;
            }
        });
    }

    fn show_profit_filter_window(&mut self, ui: &mut egui::Ui) {
        let mut open_profit = self.open_profit_filter;
        let mut keep_open = true;
        egui::Window::new("Período")
            .resizable(false)
            .collapsible(false)
            .default_width(Scale::MODAL_WIDTH_KPI)
            .max_width(Scale::MODAL_WIDTH_KPI)
            .max_height(Scale::MODAL_HEIGHT_KPI)
            .anchor(Align2::RIGHT_TOP, Vec2::new(-20.0, 150.0))
            .open(&mut open_profit)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Data Inicial:");
                    DatePickerButton::new(&mut self.profit_filter_start_date)
                        .id_salt("data_ini")
                        .ui(ui);
                });

                ui.horizontal(|ui| {
                    ui.label("Data Final:  ");
                    DatePickerButton::new(&mut self.profit_filter_end_date)
                        .id_salt("data_fim")
                        .ui(ui);
                });

                ui.vertical_centered(|ui| {
                    let mut clicked = false;
                    ui.add_enabled_ui(!self.loading, |ui| {
                        if Components::primary_button(ui, "Aplicar").clicked() {
                            clicked = true;
                        }
                    });
                    if clicked {
                        let start_chrono = NaiveDate::from_ymd_opt(
                            self.profit_filter_start_date.year() as i32,
                            self.profit_filter_start_date.month() as u32,
                            self.profit_filter_start_date.day() as u32,
                        )
                        .unwrap();
                        let end_chrono = NaiveDate::from_ymd_opt(
                            self.profit_filter_end_date.year() as i32,
                            self.profit_filter_end_date.month() as u32,
                            self.profit_filter_end_date.day() as u32,
                        )
                        .unwrap();
                        self.requested_range = Some((start_chrono, end_chrono));
                        self.loading = true;
                        keep_open = false;
                    }
                });
            });

        self.open_profit_filter = open_profit && keep_open;
    }
}
