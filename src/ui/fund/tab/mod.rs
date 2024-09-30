use crate::{message, ui::tabs::Tab};
pub mod dashboard;
use super::panel::{self, portfolio::PortfolioUI, profit::ProfitUI};
use egui::{Frame, Ui, WidgetText};
use polars::frame::DataFrame;
use tokio::sync::mpsc::UnboundedSender;

#[derive(PartialEq, Eq, Clone)]
pub enum Panel {
    Details,
    Profit,
    Assets,
}

impl Default for Panel {
    fn default() -> Self {
        Self::Details
    }
}

pub struct FundTab {
    pub title: String,
    pub fund: DataFrame,
    pub open_panel: Panel,
    pub sender: Option<UnboundedSender<message::Message>>,
    pub profit_ui: ProfitUI,
    pub portfolio_ui: PortfolioUI,
}

impl Default for FundTab {
    fn default() -> Self {
        FundTab {
            title: String::from(""),
            open_panel: Panel::default(),
            fund: DataFrame::empty(),
            sender: None,
            profit_ui: ProfitUI::default(),
            portfolio_ui: PortfolioUI::default(),
        }
    }
}

impl FundTab {
    pub fn new(title: String, fund: DataFrame, sender: UnboundedSender<message::Message>) -> Self {
        let profit_ui = ProfitUI {
            sender: Some(sender.clone()),
            cnpj: title.clone(),
            ..Default::default()
        };

        let mut portfolio_ui = PortfolioUI {
            sender: Some(sender.clone()),
            cnpj: title.clone(),
            ..Default::default()
        };

        fund.clone()
            .column("DT_INI_SIT")
            .ok()
            .and_then(|col| col.get(0).ok())
            .and_then(|val| val.get_str().map(|s| s.to_string()))
            .map(|v| {
                portfolio_ui.start_date = v;
            })
            .unwrap_or_else(|| {
                portfolio_ui.start_date = "".to_string();
            });

        FundTab {
            title,
            fund,
            sender: Some(sender),
            portfolio_ui,
            profit_ui,
            ..Default::default()
        }
    }

    fn sender(&self) -> UnboundedSender<message::Message> {
        self.sender.clone().unwrap()
    }

    pub fn set_cdi_dataframe(&mut self, df: DataFrame) {
        self.profit_ui.cdi = df;
    }

    pub fn set_ibov_dataframe(&mut self, df: DataFrame) {
        self.profit_ui.ibov = df;
    }

    pub fn set_profit_dataframe(&mut self, df: DataFrame) {
        self.profit_ui.profit = df;
    }

    pub fn set_assets_dataframe(&mut self, df: DataFrame) {
        self.portfolio_ui.assets = df;
    }

    pub fn set_top_assets_dataframe(&mut self, df: DataFrame) {
        self.portfolio_ui.top_assets = df;
    }

    pub fn set_pl_dataframe(&mut self, df: DataFrame) {
        self.portfolio_ui.pl = df;
    }

    pub fn set_profit_loading(&mut self, value: bool) {
        self.profit_ui.loading = value;
    }

    pub fn set_assets_loading(&mut self, value: bool) {
        self.portfolio_ui.loading = value;
    }
}

impl Tab for FundTab {
    fn title(&self) -> WidgetText {
        self.title.clone().into()
    }

    fn closeable(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        let _sender = self.sender().clone();
        egui::TopBottomPanel::top(ui.id().with("fund_tab_bottom_panel")).show_inside(ui, |ui| {
            if let Ok(s) = self.fund.column("DENOM_SOCIAL") {
                ui.heading(s.get(0).unwrap().get_str().unwrap());
            }
            ui.horizontal(|ui| {
                display_column_value(ui, "CNPJ:", "CNPJ_FUNDO", &self.fund);
                ui.separator();
                display_column_value(ui, "Administrador:", "ADMIN", &self.fund);
            });
        });
        ui.add_space(5.0);

        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.open_panel,
                Panel::Details,
                format!("{} Detalhes", egui_phosphor::regular::NOTE),
            );

            if ui
                .selectable_value(
                    &mut self.open_panel,
                    Panel::Profit,
                    format!("{} Rentabilidade", egui_phosphor::regular::CHART_LINE_UP),
                )
                .clicked()
                && self.profit_ui.profit.is_empty()
            {
                self.profit_ui.send_profit_message(
                    self.title().text().to_string().as_str(),
                    self.profit_ui.profit_filter_start_date,
                    self.profit_ui.profit_filter_end_date,
                )
            }

            if ui
                .selectable_value(
                    &mut self.open_panel,
                    Panel::Assets,
                    format!("{} Carteira", egui_phosphor::regular::WALLET),
                )
                .clicked()
                && self.portfolio_ui.assets.is_empty()
            {
                self.portfolio_ui.send_assets_message();
            }
        });

        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                ui.cursor().min + egui::vec2(0.0, -ui.spacing().item_spacing.y),
                egui::vec2(ui.available_width(), 2.0),
            ),
            0.0,
            ui.visuals().selection.bg_fill,
        );

        Frame::none().inner_margin(30.0).show(ui, |ui| {
            let h = ui.available_height();
            //  ui.set_min_height(h);
            ui.set_max_height(h);

            match self.open_panel {
                Panel::Details => {
                    panel::detail::show_ui(self.fund.clone(), ui);
                }
                Panel::Profit => {
                    self.profit_ui.show(ui);
                }
                Panel::Assets => {
                    self.portfolio_ui.show(ui);
                }
            };
        });
    }
}

fn display_column_value(ui: &mut Ui, label: &str, column_name: &str, fund: &DataFrame) {
    ui.label(label);
    if let Ok(column) = fund.column(column_name) {
        if let Ok(value) = column.get(0) {
            if let Some(str_value) = value.get_str() {
                ui.weak(str_value);
                return;
            }
        }
    }
    ui.weak("-");
}
