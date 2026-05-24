use std::sync::{Arc, Mutex};

use egui::{Color32, RichText, Ui, WidgetText};
use egui_toast::{Toast, ToastKind, ToastOptions};
use fundos_common::types::{FundDetail, PortfolioResponse, ProfitResponse};

use crate::ui::{
    design::Scale,
    fund::panel::{
        portfolio::PortfolioUI,
        profit::ProfitUI,
    },
    tabs::Tab,
};
use crate::util;

pub mod ativos;
pub mod dashboard;
pub mod historico;

use self::historico::HistoricoTab;

#[derive(PartialEq, Eq, Clone, Default)]
pub enum Panel {
    #[default]
    Details,
    Profit,
    Assets,
    Historico,
}

#[derive(Clone)]
pub struct FundTab {
    pub title: String,
    pub cnpj: String,
    pub fund_name: String,
    pub fund_class: String,
    pub fund_detail: Option<FundDetail>,
    pub open_panel: Panel,

    // Sub-panels
    pub profit_ui: ProfitUI,
    pub portfolio_ui: PortfolioUI,
    pub historico_tab: Option<HistoricoTab>,

    // Loading state
    pub loading: bool,
    pub loading_status: String,

    // Async pending
    pending_detail: Option<Arc<Mutex<Option<FundDetail>>>>,
    pending_profit: Option<Arc<Mutex<Option<ProfitResponse>>>>,
    pending_portfolio: Option<Arc<Mutex<Option<PortfolioResponse>>>>,
    pending_yahoo: Option<Arc<Mutex<Option<Vec<fundos_common::types::YahooPricePoint>>>>>,
    pending_holders: Option<Arc<Mutex<Option<Vec<fundos_common::types::AssetHolder>>>>>,
    repaint_ctx: Option<egui::Context>,
    initial_load_triggered: bool,
}

impl Default for FundTab {
    fn default() -> Self {
        FundTab {
            title: String::new(),
            cnpj: String::new(),
            fund_name: String::new(),
            fund_class: String::new(),
            fund_detail: None,
            open_panel: Panel::default(),
            profit_ui: ProfitUI::default(),
            portfolio_ui: PortfolioUI::default(),
            historico_tab: None,
            loading: false,
            loading_status: String::new(),
            pending_detail: None,
            pending_profit: None,
            pending_portfolio: None,
            pending_yahoo: None,
            pending_holders: None,
            repaint_ctx: None,
            initial_load_triggered: false,
        }
    }
}

impl FundTab {
    pub fn new(cnpj: String) -> Self {
        FundTab {
            title: cnpj.clone(),
            cnpj,
            ..Default::default()
        }
    }

    /// Check all pending async results. Called at start of ui().
    fn poll_pending(&mut self) {
        // pending_detail
        if let Some(pending) = self.pending_detail.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(detail) = data {
                let name = detail.denom_social.clone();
                self.fund_name = detail.denom_social.clone();
                self.fund_class = detail.classe.clone();
                self.title = detail.denom_social.clone();
                self.fund_detail = Some(detail);
                self.loading = false;
                crate::util::toaster().add(Toast {
                    kind: ToastKind::Success,
                    text: format!("Fundo carregado: {}", name).into(),
                    options: ToastOptions::default().duration_in_seconds(3.0),
                    style: Default::default(),
                });
            } else {
                self.pending_detail = Some(pending);
            }
        }

        // pending_profit
        if let Some(pending) = self.pending_profit.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(resp) = data {
                self.profit_ui.cnpj = resp.cnpj.clone();
                self.profit_ui.fund_series = resp.fund_series;
                self.profit_ui.cdi_series = resp.cdi_series;
                self.profit_ui.ibov_series = resp.ibov_series;
                self.profit_ui.loading = false;
            } else {
                self.pending_profit = Some(pending);
            }
        }

        // pending_portfolio
        if let Some(pending) = self.pending_portfolio.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(resp) = data {
                self.portfolio_ui.cnpj = resp.cnpj.clone();
                self.portfolio_ui.assets = resp.assets;
                self.portfolio_ui.top_assets = resp.top_assets;
                self.portfolio_ui.pl = resp.patrimonio_liquido;
                self.portfolio_ui.start_date = format!("{}-{}-01", resp.reference_year, resp.reference_month);
                self.portfolio_ui.loading = false;
            } else {
                self.pending_portfolio = Some(pending);
            }
        }

        // pending_yahoo — deliver to portfolio's asset_modal
        if let Some(pending) = self.pending_yahoo.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(prices) = data {
                self.portfolio_ui.asset_modal.yahoo_prices = Some(prices);
                self.portfolio_ui.asset_modal.yahoo_loading = false;
            } else {
                self.pending_yahoo = Some(pending);
            }
        }

        // pending_holders — deliver to portfolio's asset_modal
        if let Some(pending) = self.pending_holders.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(holders) = data {
                self.portfolio_ui.asset_modal.holders_data = Some(holders);
                self.portfolio_ui.asset_modal.holders_loading = false;
            } else {
                self.pending_holders = Some(pending);
            }
        }
    }

    /// Trigger initial data loads (fund detail + profit for 6M).
    fn trigger_initial_loads(&mut self) {
        if self.initial_load_triggered {
            return;
        }
        self.initial_load_triggered = true;
        self.loading = true;
        self.loading_status = "Carregando dados do fundo...".to_string();

        let cnpj = self.cnpj.clone();
        let ctx = self.repaint_ctx.clone();

        // Fund detail
        {
            let result: Arc<Mutex<Option<FundDetail>>> = Arc::new(Mutex::new(None));
            let r = result.clone();
            self.pending_detail = Some(result);
            let c = cnpj.clone();
            let ctx_c = ctx.clone();
            util::spawn_future(async move {
                match crate::api_client::get_fund_detail(&c).await {
                    Ok(detail) => *r.lock().unwrap() = Some(detail),
                    Err(e) => log::error!("Fund detail error: {}", e),
                }
                if let Some(ctx) = ctx_c {
                    ctx.request_repaint();
                }
            });
        }

        // Initial profit (6 months)
        {
            let result: Arc<Mutex<Option<ProfitResponse>>> = Arc::new(Mutex::new(None));
            let r = result.clone();
            self.pending_profit = Some(result);
            let c = cnpj.clone();
            let ctx_c = ctx.clone();
            util::spawn_future(async move {
                let end = chrono::Local::now().naive_local().date();
                let start = end
                    .checked_sub_months(chrono::Months::new(6))
                    .unwrap_or(end - chrono::Duration::days(180));
                match crate::api_client::get_profit(&c, start, end).await {
                    Ok(resp) => *r.lock().unwrap() = Some(resp),
                    Err(e) => log::error!("Profit error: {}", e),
                }
                if let Some(ctx) = ctx_c {
                    ctx.request_repaint();
                }
            });
        }
    }

    /// Handle child panel requests (portfolio date, profit range, yahoo, holders).
    fn handle_child_requests(&mut self) {
        let ctx = self.repaint_ctx.clone();

        // Portfolio date request
        if let Some((year, month)) = self.portfolio_ui.requested_date.take() {
            let cnpj = self.cnpj.clone();
            let result: Arc<Mutex<Option<PortfolioResponse>>> = Arc::new(Mutex::new(None));
            let r = result.clone();
            self.pending_portfolio = Some(result);
            let ctx_c = ctx.clone();
            util::spawn_future(async move {
                match crate::api_client::get_portfolio(&cnpj, &year, &month).await {
                    Ok(resp) => *r.lock().unwrap() = Some(resp),
                    Err(e) => log::error!("Portfolio error: {}", e),
                }
                if let Some(ctx) = ctx_c {
                    ctx.request_repaint();
                }
            });
        }

        // Profit range request
        if let Some((start, end)) = self.profit_ui.requested_range.take() {
            let cnpj = self.cnpj.clone();
            let result: Arc<Mutex<Option<ProfitResponse>>> = Arc::new(Mutex::new(None));
            let r = result.clone();
            self.pending_profit = Some(result);
            let ctx_c = ctx.clone();
            util::spawn_future(async move {
                match crate::api_client::get_profit(&cnpj, start, end).await {
                    Ok(resp) => *r.lock().unwrap() = Some(resp),
                    Err(e) => log::error!("Profit error: {}", e),
                }
                if let Some(ctx) = ctx_c {
                    ctx.request_repaint();
                }
            });
        }

        // Yahoo request from portfolio's asset modal
        if self.portfolio_ui.asset_modal.request_yahoo {
            self.portfolio_ui.asset_modal.request_yahoo = false;
            let codigo = self.portfolio_ui.asset_modal.codigo.clone();
            let result: Arc<Mutex<Option<Vec<fundos_common::types::YahooPricePoint>>>> =
                Arc::new(Mutex::new(None));
            let r = result.clone();
            self.pending_yahoo = Some(result);
            let ctx_c = ctx.clone();
            util::spawn_future(async move {
                let end = chrono::Local::now().naive_local().date();
                let start = end
                    .checked_sub_months(chrono::Months::new(24))
                    .unwrap_or(end - chrono::Duration::days(730));
                match crate::api_client::get_yahoo_prices(&codigo, start, end).await {
                    Ok(resp) => *r.lock().unwrap() = Some(resp.prices),
                    Err(e) => log::error!("Yahoo error: {}", e),
                }
                if let Some(ctx) = ctx_c {
                    ctx.request_repaint();
                }
            });
        }

        // Holders request from portfolio's asset modal
        if self.portfolio_ui.asset_modal.request_holders {
            self.portfolio_ui.asset_modal.request_holders = false;
            let codigo = self.portfolio_ui.asset_modal.codigo.clone();
            let result: Arc<Mutex<Option<Vec<fundos_common::types::AssetHolder>>>> =
                Arc::new(Mutex::new(None));
            let r = result.clone();
            self.pending_holders = Some(result);
            let ctx_c = ctx.clone();
            util::spawn_future(async move {
                let end = chrono::Local::now().naive_local().date();
                let start = end
                    .checked_sub_months(chrono::Months::new(24))
                    .unwrap_or(end - chrono::Duration::days(730));
                match crate::api_client::get_asset_holders(&codigo, start, end).await {
                    Ok(resp) => *r.lock().unwrap() = Some(resp.holders),
                    Err(e) => log::error!("Holders error: {}", e),
                }
                if let Some(ctx) = ctx_c {
                    ctx.request_repaint();
                }
            });
        }
    }

    /// Render the details panel — fund metadata.
    fn render_details(&mut self, ui: &mut Ui) {
        if self.loading && self.fund_detail.is_none() {
            crate::ui::loading::show(ui, &self.loading_status);
            return;
        }
        if let Some(ref detail) = self.fund_detail {
            let dark = ui.visuals().dark_mode;
            let hd = if dark { Color32::WHITE } else { Color32::from_rgb(20, 25, 35) };
            let tx = if dark {
                Color32::from_rgb(180, 190, 205)
            } else {
                Color32::from_rgb(70, 75, 90)
            };

            ui.group(|ui| {
                ui.label(RichText::new("Detalhes do Fundo").size(Scale::DEFAULT.heading_3()).strong().color(hd));
                ui.separator();
                ui.add_space(4.0);

                let rows: Vec<(&str, &str)> = vec![
                    ("CNPJ", &detail.cnpj),
                    ("Denominação Social", &detail.denom_social),
                    ("Classe", &detail.classe),
                    ("Situação", &detail.situacao),
                    ("Administrador", &detail.administrador),
                    ("Gestor", &detail.gestor),
                    ("Auditor", &detail.auditor),
                    ("Custodiante", &detail.adm_custodiante),
                    ("Data de Início", &detail.dt_inicio),
                    ("Data Início Situação", &detail.dt_inicio_sit),
                    ("Taxa de Administração", &detail.taxa_adm),
                    ("Taxa de Performance", &detail.taxa_perf),
                    ("Patrimônio Líquido", &detail.valor_patrim_liq),
                    ("Cotistas", &detail.cotistas_total),
                    ("Rentabilidade do Fundo", &detail.rentab_fundo),
                ];

                for (label, value) in rows {
                    if value.is_empty() || value == "0" {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}:", label))
                                .size(Scale::DEFAULT.label())
                                .color(tx),
                        );
                        ui.label(RichText::new(value).size(Scale::DEFAULT.label()).strong());
                    });
                }
            });
        }
    }
}

impl Tab for FundTab {
    fn title(&self) -> WidgetText {
        if !self.fund_name.is_empty() {
            WidgetText::from(self.fund_name.clone())
        } else {
            WidgetText::from(self.title.clone())
        }
    }

    fn closeable(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        self.repaint_ctx = Some(ui.ctx().clone());
        self.poll_pending();
        self.trigger_initial_loads();
        self.handle_child_requests();

        // Top info bar — fund name + CNPJ + Administrator
        {
            let heading_text = if !self.fund_name.is_empty() {
                &self.fund_name
            } else {
                &self.cnpj
            };
            ui.heading(RichText::new(heading_text).size(Scale::DEFAULT.heading_2()));
            ui.horizontal(|ui| {
                ui.label("CNPJ:");
                ui.weak(&self.cnpj);
                ui.separator();
                if let Some(ref detail) = self.fund_detail {
                    ui.label("Administrador:");
                    ui.weak(&detail.administrador);
                }
            });
        }
        ui.add_space(5.0);

        // Panel selector buttons with icons
        ui.horizontal(|ui| {
            let icons: &[(Panel, &str)] = &[
                (Panel::Details, egui_phosphor::regular::NOTE),
                (Panel::Profit, egui_phosphor::regular::CHART_LINE_UP),
                (Panel::Assets, egui_phosphor::regular::WALLET),
                (Panel::Historico, egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE),
            ];

            for (panel, icon) in icons {
                let selected = self.open_panel == *panel;
                let label = RichText::new(format!("{} {}", icon, match panel {
                    Panel::Details => "Detalhes",
                    Panel::Profit => "Rentabilidade",
                    Panel::Assets => "Carteira",
                    Panel::Historico => "Histórico",
                }))
                .size(Scale::DEFAULT.label());

                if ui.selectable_label(selected, label).clicked() {
                    self.open_panel = panel.clone();
                }
            }
        });

        // Active panel indicator bar
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                ui.cursor().min + egui::vec2(0.0, -ui.spacing().item_spacing.y),
                egui::vec2(ui.available_width(), 2.0),
            ),
            0.0,
            ui.visuals().selection.bg_fill,
        );
        ui.add_space(4.0);

        match self.open_panel {
            Panel::Details => self.render_details(ui),
            Panel::Profit => {
                self.profit_ui.show(ui);
            }
            Panel::Assets => {
                self.portfolio_ui.show(ui);
            }
            Panel::Historico => {
                if self.historico_tab.is_none() {
                    self.historico_tab = Some(HistoricoTab::new(self.cnpj.clone()));
                }
                if let Some(ref mut hist) = self.historico_tab {
                    hist.ui(ui);
                }
            }
        }
    }
}
