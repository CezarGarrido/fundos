use egui::FontId;
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabAddAlign};
use polars::{error::PolarsError, frame::DataFrame};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    config::watch,
    history::History,
    message::Message,
    provider::{
        cvm::{
            self,
            fund::{self, Register},
            informe::{self, Informe},
            portfolio::{self, Portfolio},
        },
        downloader::{self, DownloadStatus},
        indices::{self},
    },
    ui::{
        fund::{
            modal::{asset::AssetDetail, search::Search},
            tab::{dashboard::DashboardTab, FundTab},
        },
        modal::{self, about::About},
        tabs::{home_tab::HomeTab, Tab, TabType, TabViewer},
    },
    util,
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
    config_modal: modal::config::Config,

    #[serde(skip)]
    started_watch: bool,

    #[serde(skip)]
    pub status: String,

    #[serde(skip)]
    is_running_download: bool,
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

        let mut register = Register::new();
        if register.load().is_err() {
            log::error!("Erro ao carregar cadastro");
        }

        let informe: Informe = Informe::new();
        let portfolio = Portfolio::new();

        let initial_funds = register
            .find(None, None, None, Some(20))
            .unwrap_or(DataFrame::empty());

        let s = channel.0.clone();
        let mut search = Search::new(false, s.clone());

        search.set_result(initial_funds);

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
            config_modal: modal::config::Config::new(s.clone()),
            started_watch: false,
            status: String::from(""),
            is_running_download: false,
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

    fn start_watch(&mut self, ctx: &egui::Context) {
        if self.started_watch {
            return;
        }

        let ctx = ctx.clone();
        let sender = self.channel.0.clone();
        tokio::spawn(async move {
            watch(&ctx, sender);
        });

        self.started_watch = true;
    }

    fn start_download(&mut self, key: String) -> CancellationToken {
        let token = CancellationToken::new();
        self.downloads.insert(key, token.clone());
        token
    }

    fn cancel_download(&mut self, key: String) {
        if let Some(token) = self.downloads.remove(&key) {
            token.cancel();
        }
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

    fn handle_messages(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let ctxc = ctx.clone();
        let sender = self.channel.0.clone();

        if let Ok(message) = self.channel.1.try_recv() {
            match message {
                Message::ProgressDownload(group, index, dl) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Home(stb) = tb {
                            //downloader.update_download(group.as_str(), index, dl);
                            ctx.request_repaint(); // Solicita um redesenho da interface
                            break;
                        }
                    }
                }
                Message::CancelDownload(key) => {
                    self.cancel_download(key);
                }
                Message::StartDownload(group, index, download_item) => {}
                Message::OpenSearchWindow(value) => {
                    self.asset_detail_modal.open_window = false;
                    self.search.open(value)
                }
                Message::NewTab(cnpj) => {
                    let res = self.register.find_by_cnpj(cnpj.clone());
                    match res {
                        Ok(fund_dataframe) => {
                            self.tab_viewer.open_window = false;
                            self.search.open(false);
                            self.add_tab(cnpj, fund_dataframe);
                        }
                        Err(err) => {
                            log::error!("Erro ao carregar fundo {}", err);
                        }
                    }
                }
                Message::Profit(cnpj, start_date, end_date) => {
                    let informe = self.informe.clone();
                    tokio::spawn(async move {
                        let cdi_future = async { indices::cdi::dataframe(start_date, end_date) };
                        let ibov_future =
                            async { indices::ibovespa::dataframe(start_date, end_date) };
                        let profitability_future =
                            async { informe.profitability(cnpj.clone(), start_date, end_date) };
                        let (profitability_result, cdi_result, ibov_result) =
                            tokio::join!(profitability_future, cdi_future, ibov_future);
                        let cdi_dataframe = handle_result(cdi_result);
                        let profitability_dataframe = handle_result(profitability_result);
                        let ibov_dataframe = handle_result(ibov_result);
                        if let Err(e) = sender.send(Message::ProfitResult(
                            cnpj.clone(),
                            profitability_dataframe,
                            cdi_dataframe,
                            ibov_dataframe,
                        )) {
                            log::error!("Falha ao enviar mensagem: {}", e);
                        }
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
                        let pl = match portfolio.patrimonio_liquido(
                            cnpj.clone(),
                            year.clone(),
                            month.clone(),
                        ) {
                            Ok(df) => df,
                            Err(e) => {
                                log::error!("Erro ao obter patrimônio líquido: {:?}", e);
                                return; // Não envie a mensagem se ocorrer erro
                            }
                        };

                        let (assets, top_assets) = match portfolio.assets(
                            pl.clone(),
                            cnpj.clone(),
                            year.clone(),
                            month.clone(),
                            true,
                        ) {
                            Ok(dfs) => dfs,
                            Err(e) => {
                                log::error!("Erro ao obter ativos: {:?}", e);
                                return; // Não envie a mensagem se ocorrer erro
                            }
                        };

                        if let Err(e) =
                            sender.send(Message::AssetsResult(cnpj.clone(), assets, top_assets, pl))
                        {
                            log::error!("Erro ao enviar mensagem: {:?}", e);
                        }

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
                    let register = self.register.clone();
                    tokio::spawn(async move {
                        let res = register.find(Some(keyword), class, None, None);
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
                        let result = r.stats();
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
                Message::RefreshConfig => {
                    if !self.is_running_download {
                        tokio::spawn(async move {
                            downloader::download_all(
                                CancellationToken::new(),
                                move |dl| {
                                    match dl {
                                        DownloadStatus::InProgress(pg) => {
                                            let _ = sender.send(Message::UpdateStatus(pg));
                                        }
                                        _ => {}
                                    }
                                    ctxc.request_repaint();
                                },
                                25,
                            );
                        });
                        self.is_running_download = true;
                    }
                }
                Message::UpdateStatus(msg) => {
                    self.status = msg;
                }
            }
        }
    }

    fn get_missing_files(&self) -> Vec<String> {
        let mut missing_files = Vec::new();

        // Carrega as opções de cada módulo
        let fund_opts = fund::options::load().unwrap();
        let portfolio_opts = portfolio::options::load().unwrap();
        let informe_opts = informe::options::load().unwrap();
        let cdi_opts = indices::cdi::options::load().unwrap();
        let ibov_opts = indices::ibovespa::options::load().unwrap();

        // Verifica se os caminhos existem e adiciona os arquivos faltantes à lista
        if !fund_opts.path.exists() {
            missing_files.push(fund_opts.path.to_str().unwrap().to_string());
        }
        if !portfolio_opts.path.exists() {
            missing_files.push(portfolio_opts.path.to_str().unwrap().to_string());
        }
        if !informe_opts.path.exists() {
            missing_files.push(informe_opts.path.to_str().unwrap().to_string());
        }
        if !cdi_opts.path.exists() {
            missing_files.push(cdi_opts.path.to_str().unwrap().to_string());
        }
        if !ibov_opts.path.exists() {
            missing_files.push(ibov_opts.path.to_str().unwrap().to_string());
        }

        missing_files
    }

    fn start_indexing() {}
}

impl eframe::App for TemplateApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.start_watch(ctx);

        self.handle_messages(ctx, frame);

        let missing_files = self.get_missing_files();
        util::toaster::toaster().show(ctx);
        if !missing_files.is_empty() {
            self.config_modal.initial = true;
            //self.config_modal.open(true);
        } else {
            self.config_modal.open(false);
        }

        //  self.config_modal.show(ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.set_enabled(!self.config_modal.show);
            egui::menu::bar(ui, |ui| {
                let font_id = FontId::proportional(16.0);

                let txt =
                    egui::RichText::new(egui_phosphor::regular::LIST.to_string()).font(font_id);

                ui.menu_button(txt, |ui| {
                    ui.menu_button("Fundo", |ui| {
                        if ui.button("Pesquisar").clicked() {
                            let _ = self.channel.0.send(Message::OpenSearchWindow(true));
                        }
                    });

                    if ui.button("Sobre").clicked() {
                        self.about_modal.open(true);
                        // ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    ui.separator();
                    if ui.button("Sair").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);
            });
        });

        self.show_statusbar(ctx, frame);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(!self.config_modal.show);
            self.search.show(ui);
            self.asset_detail_modal.show(ui);
            self.about_modal.show(ui);

            egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
                if !self.config_modal.show {
                    DockArea::new(&mut self.tree)
                        .style({
                            let mut style = Style::from_egui(ctx.style().as_ref());
                            // style.tab_bar.fill_tab_bar = true;
                            style.buttons.add_tab_align = TabAddAlign::Left;
                            style
                        })
                        .show_add_buttons(true)
                        .show_inside(ui, &mut self.tab_viewer);
                }
            });
        });
    }
}

// Função auxiliar para transformar Result em DataFrame
fn handle_result(result: Result<DataFrame, PolarsError>) -> DataFrame {
    match result {
        Ok(df) => df,
        Err(e) => {
            log::error!("Falha ao carregar dados: {}", e);
            DataFrame::empty()
        }
    }
}
