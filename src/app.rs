use crate::{
    provider::cvm::{fund, informe},
    util,
};
use egui::FontId;
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabAddAlign};
use egui_toast::{Toast, ToastOptions};
use polars::{error::PolarsError, frame::DataFrame};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    history::History,
    message::Message,
    provider::{
        cvm::{fund::Register, informe::Informe, portfolio::Portfolio},
        indices::{self},
    },
    ui::{
        fund::{
            modal::{asset::AssetDetail, search::Search},
            tab::{dashboard::DashboardTab, FundTab},
        },
        modal::about::About,
        tabs::{config_tab::ConfigTab, home_tab::HomeTab, Tab, TabType, TabViewer},
    },
};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    #[serde(skip)]
    tab_viewer: TabViewer,
    #[serde(skip)]
    tree: DockState<TabType>,
    #[serde(skip)]
    pub channel: (
        mpsc::UnboundedSender<Message>,
        mpsc::UnboundedReceiver<Message>,
    ),
    #[serde(skip)]
    history: History,
    #[serde(skip)]
    register: Register,
    #[serde(skip)]
    informe: Informe,
    #[serde(skip)]
    portfolio: Portfolio,
    #[serde(skip)]
    downloads: HashMap<String, CancellationToken>,
    #[serde(skip)]
    search: Search,
    pub open_logs: bool,
    #[serde(skip)]
    asset_detail_modal: AssetDetail,
    open_list_tab: bool,

    #[serde(skip)]
    about_modal: About,

    #[serde(skip)]
    started_watch: bool,

    #[serde(skip)]
    pub status: String,

    #[serde(skip)]
    downloading: bool,
}

impl Default for TemplateApp {
    fn default() -> Self {
        let channel = mpsc::unbounded_channel();
        let history = History::new();
        if history.load().is_err() {
            log::error!("Erro ao carregar histórico");
        }

        let tree: DockState<TabType> = DockState::new(vec![TabType::Home(HomeTab::new(
            "Início".to_string(),
            channel.0.clone(),
            history.clone(),
        ))]);

        let tab_viewer = TabViewer {
            open_window: false,
            sender: channel.0.clone(),
        };

        let register = Register::new();
        let informe: Informe = Informe::new();
        let portfolio = Portfolio::new();
        let s = channel.0.clone();
        let search = Search::new(false, s.clone());

        Self {
            tree,
            tab_viewer,
            channel,
            history,
            register,
            informe,
            portfolio,
            open_logs: false,
            downloads: HashMap::new(),
            search,
            asset_detail_modal: AssetDetail {
                asset: DataFrame::empty(),
                open_window: false,
            },
            open_list_tab: false,
            about_modal: About::new(),
            started_watch: false,
            status: String::from(""),
            downloading: false,
        }
    }
}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    pub fn add_tab(&mut self, cnpj: String, df: DataFrame) {
        let tabs: Vec<_> = self
            .tree
            .iter_all_tabs()
            .map(|(_, tab)| tab.to_owned())
            .collect();

        if let Some(index) = tabs
            .iter()
            .position(|tb| tb.title().text().contains(&cnpj.clone()))
        {
            let main_surface = self.tree.main_surface_mut();
            main_surface.set_active_tab(NodeIndex(0), egui_dock::TabIndex(index));
        } else {
            let main_surface = self.tree.main_surface_mut();
            main_surface.set_focused_node(egui_dock::NodeIndex(2));
            let new_fund_tab = FundTab::new(cnpj.clone(), df, self.channel.0.clone());
            main_surface.push_to_focused_leaf(TabType::Fund(new_fund_tab));
        }
        self.history.add(cnpj.clone());
        let _ = self.history.save();
        let _ = self.history.load();
    }

    pub fn add_dashboard_tab(&mut self) {
        let tabs: Vec<_> = self
            .tree
            .iter_all_tabs()
            .map(|(_, tab)| tab.to_owned())
            .collect();

        if let Some(index) = tabs
            .iter()
            .position(|tb| tb.title().text().contains("Dashboard"))
        {
            let main_surface = self.tree.main_surface_mut();
            main_surface.set_active_tab(NodeIndex(0), egui_dock::TabIndex(index));
        } else {
            let main_surface = self.tree.main_surface_mut();
            main_surface.set_focused_node(egui_dock::NodeIndex(2));
            let dash_tab = DashboardTab {
                title: "Dashboard".to_string(),
                by_year: DataFrame::empty(),
                by_situation: DataFrame::empty(),
                by_class: DataFrame::empty(),
            };
            main_surface.push_to_focused_leaf(TabType::Dashboard(dash_tab));
        }
    }

    pub fn add_config_tab(&mut self) {
        let tabs: Vec<_> = self
            .tree
            .iter_all_tabs()
            .map(|(_, tab)| tab.to_owned())
            .collect();

        if let Some(index) = tabs
            .iter()
            .position(|tb| tb.title().text().contains("Configuração"))
        {
            let main_surface = self.tree.main_surface_mut();
            main_surface.set_active_tab(NodeIndex(0), egui_dock::TabIndex(index));
        } else {
            let main_surface = self.tree.main_surface_mut();
            main_surface.set_focused_node(egui_dock::NodeIndex(2));
            let tab = ConfigTab::new("Configuração".to_string());
            main_surface.push_to_focused_leaf(TabType::Config(tab));
        }
    }

    fn handle_update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let ctxc = ctx.clone();
        let sender = self.channel.0.clone();

        if let Ok(message) = self.channel.1.try_recv() {
            match message {
                Message::OpenSearchWindow(value) => {
                    let _ = sender.send(Message::SearchFunds("".to_string(), None));
                    self.asset_detail_modal.open_window = false;
                    self.search.open(value)
                }
                Message::OpenTab(cnpj, df) => {
                    self.tab_viewer.open_window = false;
                    self.search.open(false);
                    self.add_tab(cnpj, df);
                }
                Message::NewTab(cnpj) => {
                    let r = self.register.clone();
                    tokio::spawn(async move {
                        let res: Result<DataFrame, fund::Error> =
                            r.async_find_by_cnpj(cnpj.clone()).await;
                        match res {
                            Ok(fund_dataframe) => {
                                let _ = sender.send(Message::OpenTab(cnpj, fund_dataframe));
                                ctxc.request_repaint();
                            }
                            Err(err) => {
                                log::error!("Erro ao carregar fundo {}", err);
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Error,
                                    text: "Erro ao obter dados do fundo".into(),
                                    options: ToastOptions::default().duration_in_seconds(3.0),
                                });
                            }
                        }
                    });
                }
                Message::Profit(cnpj, start_date, end_date) => {
                    let informe = self.informe.clone();
                    tokio::spawn(async move {
                        let cdi_future = async { indices::cdi::dataframe(start_date, end_date) };
                        let ibov_future = indices::ibovespa::async_dataframe(start_date, end_date);
                        let profitability_future =
                            informe.async_profit(cnpj.clone(), start_date, end_date);
                        let (profitability_result, cdi_result, ibov_result) =
                            tokio::join!(profitability_future, cdi_future, ibov_future);
                        let cdi_dataframe = handle_profit_result("cdi", cdi_result);
                        let profitability_dataframe =
                            handle_profit_result("fundo", profitability_result);
                        let ibov_dataframe = handle_profit_result("ibov", ibov_result);
                        let _ = sender.send(Message::ProfitResult(
                            cnpj.clone(),
                            profitability_dataframe,
                            cdi_dataframe,
                            ibov_dataframe,
                        ));
                        ctxc.request_repaint();
                    });
                }
                Message::ProfitResult(cnpj, df, cdi, ibov) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Fund(stb) = tb {
                            if *stb.title().text().to_string() == cnpj {
                                stb.set_profit_dataframe(df.clone());
                                stb.set_cdi_dataframe(cdi.clone());
                                stb.set_ibov_dataframe(ibov.clone());
                                stb.set_profit_loading(false);
                                ctx.request_repaint();
                                break;
                            }
                        }
                    }
                }
                Message::Assets(cnpj, year, month) => {
                    let portfolio = self.portfolio.clone();
                    tokio::spawn(async move {
                        let (pl, assets, top_assets) = match portfolio
                            .async_assets(cnpj.clone(), year.clone(), month.clone(), true)
                            .await
                        {
                            Ok(dfs) => dfs,
                            Err(e) => {
                                log::error!("Erro ao obter ativos: {:?}", e);
                                return; // Não envie a mensagem se ocorrer erro
                            }
                        };
                        let _ = sender.send(Message::AssetsResult(
                            cnpj.clone(),
                            assets,
                            top_assets,
                            pl,
                        ));
                        ctxc.request_repaint();
                    });
                }
                Message::AssetsResult(cnpj, assets, top_assets, patrimonio_liquido) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Fund(tab) = tb {
                            if *tab.title().text().to_string() == cnpj {
                                tab.set_assets_dataframe(assets.clone());
                                tab.set_top_assets_dataframe(top_assets.clone());
                                tab.set_pl_dataframe(patrimonio_liquido.clone());
                                ctxc.request_repaint();
                                break;
                            }
                        }
                    }
                }
                Message::ResultFunds(df) => {
                    self.search.set_result(df);
                }
                Message::SearchFunds(keyword, class) => {
                    let keyword = keyword.clone();
                    let r = self.register.clone();
                    tokio::spawn(async move {
                        let res = r.async_find(Some(keyword), class, None, None).await;
                        match res {
                            Ok(df) => {
                                let _ = sender.send(Message::ResultFunds(df));
                                ctxc.request_repaint();
                            }
                            Err(err) => {
                                log::error!("Erro ao buscar fundos {:?}", err);
                            }
                        }
                    });
                }
                Message::ShowAssetDetail(df) => {
                    self.asset_detail_modal.asset = df;
                    self.asset_detail_modal.open_window = true;
                }
                Message::OpenDashboardTab => {
                    self.add_dashboard_tab();

                    let sender = sender.clone();
                    let r = self.register.clone();
                    tokio::spawn(async move {
                        let result = r.async_stats().await;
                        match result {
                            Ok((a, b, c)) => {
                                let _ = sender.send(Message::DashboardTabResult(a, b, c));
                                ctxc.request_repaint();
                            }
                            Err(err) => {
                                log::error!("Erro ao buscar estatisticas {}", err);
                            }
                        }
                    });
                }
                Message::DashboardTabResult(a, b, c) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Dashboard(tab) = tb {
                            tab.set_dataframes(a.clone(), b.clone(), c.clone());
                            ctxc.request_repaint();
                            break;
                        }
                    }
                }
                Message::StartDownload => {
                    if !self.downloading {
                        tokio::spawn(async move {});
                        self.downloading = true;
                    }
                }
            }
        }
    }

    fn check_dependencies(&mut self, ctx: &egui::Context) {}

    // Função para configurar o painel superior
    fn setup_top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.setup_menu_bar(ui, ctx);
        });
    }

    // Função para configurar a barra de menu
    fn setup_menu_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::menu::bar(ui, |ui| {
            let font_id = FontId::proportional(16.0);
            let icon = egui::RichText::new(egui_phosphor::regular::LIST.to_string()).font(font_id);
            ui.menu_button(icon, |ui| {
                self.setup_fund_menu(ui);
                if ui.button("Configuração").clicked() {
                    self.add_config_tab();
                }
                if ui.button("Sobre").clicked() {
                    self.about_modal.open(true);
                }
                ui.separator();
                if ui.button("Sair").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }

    // Função para configurar o menu de fundos
    fn setup_fund_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Fundo", |ui| {
            if ui.button("Pesquisar").clicked() {
                let _ = self.channel.0.send(Message::OpenSearchWindow(true));
            }
        });
    }

    // Função para configurar o painel central
    fn setup_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.search.open_window {
                self.search.set_result(DataFrame::empty());
            }
            self.search.show(ui);
            self.asset_detail_modal.show(ui);
            self.about_modal.show(ui);
            self.setup_dock_area(ui, ctx);
        });
    }

    // Função para configurar a Dock Area
    fn setup_dock_area(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
            DockArea::new(&mut self.tree)
                .style({
                    let mut style = Style::from_egui(ctx.style().as_ref());
                    style.buttons.add_tab_align = TabAddAlign::Left;
                    style
                })
                .show_add_buttons(true)
                .show_inside(ui, &mut self.tab_viewer);
        });
    }
}

impl eframe::App for TemplateApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.check_dependencies(ctx);
        self.handle_update(ctx, frame);
        self.setup_top_panel(ctx);
        self.show_statusbar(ctx, frame);
        self.setup_central_panel(ctx);
        util::toaster().show(ctx);
    }
}

// Função auxiliar para transformar Result em DataFrame
fn handle_profit_result(name: &str, result: Result<DataFrame, PolarsError>) -> DataFrame {
    let msg = format!("Erro ao obter rentabilidade: {}", name);
    match result {
        Ok(df) => df,
        Err(e) => {
            util::toaster().add(Toast {
                kind: egui_toast::ToastKind::Error,
                text: msg.into(),
                options: ToastOptions::default().duration_in_seconds(3.0),
            });

            log::error!("Falha ao carregar dados: {}", e);
            DataFrame::empty()
        }
    }
}
