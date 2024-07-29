use egui_dock::{DockArea, DockState, NodeIndex, Style, TabAddAlign};
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
        fund::{modal::search::Search, tab::FundTab},
        tabs::{home_tab::HomeTab, Tab, TabType, TabViewer},
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
    channel: (
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
}

impl Default for TemplateApp {
    fn default() -> Self {
        let channel = mpsc::unbounded_channel();
        let history = History::new();
        if history.load().is_err() {
            log::error!("erro ao carregar dataframe");
        }

        let tree: DockState<TabType> = DockState::new(vec![TabType::Home(HomeTab::new(
            "Início".to_string(),
            channel.0.clone(),
            history.clone(),
        ))]);

        let tab_viewer = TabViewer { open_window: false };

        let mut register = Register::new();
        if register.load().is_err() {
            log::error!("erro ao carregar dataframe");
        }

        let informe: Informe = Informe::new();
        let portfolio = Portfolio::new();

        let initial_funds = register
            .find(None, None, None, Some(10))
            .unwrap_or(DataFrame::empty());

        let s = channel.0.clone();
        let mut search = Search::new(false, s);

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
        }
    }
}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

        cc.egui_ctx.set_fonts(fonts);
        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }
        Default::default()
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

    fn handle_messages(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let ctxc = ctx.clone();
        let sender = self.channel.0.clone();

        if let Ok(message) = self.channel.1.try_recv() {
            match message {
                Message::ProgressDownload(group, index, dl) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Home(stb) = tb {
                            stb.download_manager
                                .update_download(group.as_str(), index, dl);
                            ctx.request_repaint(); // Solicita um redesenho da interface
                            break;
                        }
                    }
                }
                Message::CancelDownload(key) => {
                    self.cancel_download(key);
                }
                Message::StartDownload(group, index, download_item) => {
                    // Crie um novo token de cancelamento e inicie o download
                    let token = { self.start_download(format!("{}_{}", group, index)) };
                    let g = group.clone();
                    tokio::spawn(async move {
                        indices::download(token.clone(), download_item.id, move |dl| {
                            let _ = sender.send(Message::ProgressDownload(g.clone(), index, dl));
                            ctxc.request_repaint(); // Wake up UI thread
                        })
                    });
                }
                Message::OpenSearchWindow(value) => {
                    // self.tab_viewer.open_window = value;
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
                            log::error!("erro ao carregar fundo {}", err);
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
                            tokio::join!(cdi_future, profitability_future, ibov_future);
                        let cdi_dataframe = handle_result(cdi_result);
                        let profitability_dataframe = handle_result(profitability_result);
                        let ibov_dataframe = handle_result(ibov_result);
                        // Envie a mensagem com os DataFrames
                        if let Err(e) = sender.send(Message::ProfitResult(
                            cnpj.clone(),
                            profitability_dataframe,
                            cdi_dataframe,
                            ibov_dataframe,
                        )) {
                            log::error!("Failed to send ProfitResult message: {}", e);
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
                                log::error!("erro ao obter patrimônio líquido: {:?}", e);
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
                                log::error!("erro ao obter ativos: {:?}", e);
                                return; // Não envie a mensagem se ocorrer erro
                            }
                        };

                        if let Err(e) =
                            sender.send(Message::AssetsResult(cnpj.clone(), assets, top_assets, pl))
                        {
                            log::error!("rrro ao enviar mensagem: {:?}", e);
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
                                log::error!("erro ao buscar fundos {:?}", err);
                            }
                        }
                    });
                }
            }
        }
    }
}

impl eframe::App for TemplateApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_messages(ctx, frame);
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Arquivo", |ui| {
                    if ui.button("Configuração").clicked() {
                        //self.config.open = !self.config.open;
                    }
                    ui.separator();
                    if ui.button("Sair").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);
                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });

        self.show_statusbar(ctx, frame);

        egui::CentralPanel::default().show(ctx, |ui| {
            self.search.show(ui);
            egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
                DockArea::new(&mut self.tree)
                    .style({
                        let mut style = Style::from_egui(ctx.style().as_ref());
                        // style.tab_bar.fill_tab_bar = true;
                        style.buttons.add_tab_align = TabAddAlign::Left;
                        style
                    })
                    .show_add_buttons(true)
                    .show_inside(ui, &mut self.tab_viewer);
            });
        });
    }
}

// Função auxiliar para transformar Result em DataFrame
fn handle_result(result: Result<DataFrame, PolarsError>) -> DataFrame {
    match result {
        Ok(df) => df,
        Err(e) => {
            log::error!("Failed to load data: {}", e);
            DataFrame::empty()
        }
    }
}
