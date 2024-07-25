use super::panel;
use super::panel::assets::PortfolioUI;
use super::panel::profit::ProfitUI;
use crate::message;
use crate::tabs::Tab;
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
        //let dates = dataframe::available_dates();

        let mut profit_ui = ProfitUI::default();
        profit_ui.sender = Some(sender.clone());
        profit_ui.cnpj = title.clone();

        let mut portfolio_ui = PortfolioUI::default();
        portfolio_ui.sender = Some(sender.clone());
        portfolio_ui.cnpj = title.clone();

        let mut d = FundTab::default();
        d.title = title;
        d.fund = fund;
        d.sender = Some(sender);
        d.portfolio_ui = portfolio_ui;
        d.profit_ui = profit_ui;

        d
    }

    fn sender(&self) -> UnboundedSender<message::Message> {
        self.sender.clone().unwrap()
    }

    pub fn set_cdi_dataframe(&mut self, df: DataFrame) {
        self.profit_ui.cdi = df;
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
}

impl Tab for FundTab {
    fn title(&self) -> WidgetText {
        self.title.clone().into()
    }

    fn closeable(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        let sender = self.sender().clone();
        egui::TopBottomPanel::top("top_bottom_panel").show_inside(ui, |ui| {
            if let Ok(s) = self.fund.column("DENOM_SOCIAL") {
                ui.heading(s.get(0).unwrap().get_str().unwrap());
            }
            ui.horizontal(|ui| {
                display_column_value(ui, "CNPJ:", "CNPJ_FUNDO", &self.fund);
                ui.separator();
                display_column_value(ui, "Gestor:", "GESTOR", &self.fund);
                ui.separator();
                display_column_value(ui, "Administrador:", "ADMIN", &self.fund);
            });
        });

        Frame::none().inner_margin(10.0).show(ui, |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.open_panel, Panel::Details, "Detalhes");

                    if ui
                        .selectable_value(&mut self.open_panel, Panel::Profit, "Rentabilidade")
                        .clicked()
                    {
                        if self.profit_ui.profit.is_empty() {
                            let _ = sender.send(message::Message::Profit(
                                self.title().text().to_string(),
                                self.profit_ui.profit_filter_start_date.clone(),
                                self.profit_ui.profit_filter_end_date.clone(),
                            ));
                        }
                    }

                    if ui
                        .selectable_value(&mut self.open_panel, Panel::Assets, "Carteira")
                        .clicked()
                    {
                        if self.portfolio_ui.assets.is_empty() {
                            let _ = sender.send(message::Message::Assets(
                                self.title().text().to_string(),
                                self.portfolio_ui.assets_filter_year.clone(),
                                self.portfolio_ui.assets_filter_month.clone(),
                            ));
                        }
                    }
                });

                ui.separator();

                Frame::none().inner_margin(30.0).show(ui, |ui| {
                    ui.set_min_height(ui.available_height());
                    match self.open_panel {
                        Panel::Details => {
                            panel::details::show_ui(self.fund.clone(), ui);
                        }
                        Panel::Profit => {
                            self.profit_ui.show(ui);
                        }
                        Panel::Assets => {
                            self.portfolio_ui.show(ui);
                        }
                    };
                });
            });
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
