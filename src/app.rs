use egui::{Align2, Vec2};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabAddAlign};
use egui_extras::{Column, TableBuilder};

use polars::frame::DataFrame;

use tokio::sync::mpsc;

use crate::{
    config::Config,
    cvm::{
        fund::{Class, Register},
        indicator,
        informe::Informe,
        portfolio::Portfolio,
    },
    history::History,
    message::Message,
    tabs::{fund_tab::fund_tab::FundTab, home_tab::HomeTab, Tab, TabType, TabViewer},
};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    #[serde(skip)] // This how you opt-out of serialization of a field
    fund_class: Option<Class>,
    #[serde(skip)]
    tab_viewer: TabViewer,
    #[serde(skip)]
    tree: DockState<TabType>,
    #[serde(skip)]
    config: Config,
    #[serde(skip)]
    query: String,
    #[serde(skip)]
    result_funds: DataFrame,
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
}

impl Default for TemplateApp {
    fn default() -> Self {
        let channel = mpsc::unbounded_channel();
        let config = Config::new(channel.0.clone());

        let history = History::new();
        let _ = history.load();

        let tree: DockState<TabType> = DockState::new(vec![TabType::Home(HomeTab::new(
            "Início".to_string(),
            config.clone(),
            channel.0.clone(),
            history.clone(),
        ))]);

        let tab_viewer = TabViewer { open_window: false };
        let register = Register::new();
        let informe: Informe = Informe::new();
        let portfolio = Portfolio::new();
        let initial_funds = register
            .find(None, None, None, Some(10))
            .unwrap_or(DataFrame::empty());

        Self {
            tree,
            tab_viewer,
            fund_class: None,
            config,
            query: "".to_owned(),
            channel,
            result_funds: initial_funds,
            history,
            register,
            informe,
            portfolio,
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
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
                Message::OpenSearchWindow(value) => {
                    self.tab_viewer.open_window = value;
                }
                Message::DownloadMessage(idx, progress) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        match tb {
                            TabType::Home(stb) => {
                                stb.config.downloader.update_download(idx, progress);
                                ctxc.request_repaint();
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Message::NewTab(cnpj) => {
                    let res = self.register.find_by_cnpj(cnpj.clone());
                    match res {
                        Ok(fund_dataframe) => {
                            self.tab_viewer.open_window = false;
                            self.add_tab(cnpj, fund_dataframe);
                        }
                        Err(err) => {
                            println!("err {}", err);
                        }
                    }
                }
                Message::Profit(cnpj, start_date, end_date) => {
                    let informe = self.informe.clone();
                    tokio::spawn(async move {
                        let cdi_future =
                            async { indicator::cdi(start_date.clone(), end_date.clone()) };
                        let profitability_future = async {
                            informe.profitability(
                                cnpj.clone(),
                                start_date.clone(),
                                end_date.clone(),
                            )
                        };
                        let (cdi_result, profitability_result) =
                            tokio::join!(cdi_future, profitability_future);
                        match (cdi_result, profitability_result) {
                            (Ok(cdi_dataframe), Ok(profit_dataframe)) => {
                                let _ = sender.send(Message::ProfitResult(
                                    cnpj,
                                    profit_dataframe,
                                    cdi_dataframe,
                                ));
                                ctxc.request_repaint();
                            }
                            (Ok(cdi_dataframe), Err(err)) => {
                                let _ = sender.send(Message::ProfitResult(
                                    cnpj.clone(),
                                    DataFrame::empty(), // Placeholder for empty DataFrame
                                    cdi_dataframe,
                                ));
                                println!("Profitability error: {}", err);
                                ctxc.request_repaint();
                            }
                            (Err(err), Ok(profit_dataframe)) => {
                                let _ = sender.send(Message::ProfitResult(
                                    cnpj.clone(),
                                    profit_dataframe,
                                    DataFrame::empty(), // Placeholder for empty DataFrame
                                ));
                                println!("CDI error: {}", err);
                                ctxc.request_repaint();
                            }
                            (Err(err1), Err(err2)) => {
                                println!("Both errors: {}, {}", err1, err2);
                            }
                        }
                    });
                }
                Message::ProfitResult(cnpj, df, cdi) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        match tb {
                            TabType::Fund(stb) => {
                                if stb.title().text().to_string() == cnpj {
                                    stb.set_profit_dataframe(df.clone());
                                    stb.set_cdi_dataframe(cdi.clone());
                                    stb.set_profit_loading(false);
                                    ctx.request_repaint();
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Message::Assets(cnpj, year, month) => {
                    println!("cnpj {} ano {} mes {}", cnpj, year, month);
                    let portfolio = self.portfolio.clone();
                    tokio::spawn(async move {
                        let pl = match portfolio.patrimonio_liquido(
                            cnpj.clone(),
                            year.clone(),
                            month.clone(),
                        ) {
                            Ok(df) => df,
                            Err(e) => {
                                eprintln!("Erro ao obter patrimônio líquido: {:?}", e);
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
                                eprintln!("Erro ao obter ativos: {:?}", e);
                                return; // Não envie a mensagem se ocorrer erro
                            }
                        };

                        if let Err(e) =
                            sender.send(Message::AssetsResult(cnpj.clone(), assets, top_assets, pl))
                        {
                            eprintln!("Erro ao enviar mensagem: {:?}", e);
                        }

                        ctxc.request_repaint();
                    });
                }
                Message::AssetsResult(cnpj, assets, top_assets, patrimonio_liquido) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        match tb {
                            TabType::Fund(tab) => {
                                if tab.title().text().to_string() == cnpj {
                                    tab.set_assets_dataframe(assets.clone());
                                    tab.set_top_assets_dataframe(top_assets.clone());
                                    tab.set_pl_dataframe(patrimonio_liquido.clone());
                                    ctxc.request_repaint();
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Message::ResultFunds(df) => {
                    self.result_funds = df;
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
                                log::error!("{}", err); // TODO: tratar erro
                            }
                        }
                    });
                }
            }
        }
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_messages(ctx, frame);
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("Arquivo", |ui| {
                        if ui.button("Configuração").clicked() {
                            self.config.open = !self.config.open;
                        }
                        ui.separator();
                        if ui.button("Sair").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }
                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });
        self.config.show(ctx);
        self.show_statusbar(ctx, frame);

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Window::new("Fundos")
                .resizable(false)
                .collapsible(false)
                .default_width(550.0)
                .max_width(550.0)
                .max_height(700.0)
                .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 150.0))
                .open(&mut self.tab_viewer.open_window)
                .show(ctx, |ui| {
                    let search_bar = egui::TextEdit::singleline(&mut self.query)
                        .font(egui::TextStyle::Body)
                        .hint_text("🔍 Busque pelo nome ou cnpj do fundo..")
                        .frame(true)
                        .desired_width(ui.available_width())
                        .margin(egui::vec2(15.0, 10.0));

                    let search_response: egui::Response = ui.add(search_bar);
                    if search_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        send_find_message(
                            self.channel.0.clone(),
                            &self.query,
                            self.fund_class.clone(),
                        )
                    }

                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        handle_selectable_value(
                            ui,
                            &mut self.fund_class,
                            None,
                            "Todos",
                            self.channel.0.clone(),
                            &self.query,
                        );
                        handle_selectable_value(
                            ui,
                            &mut self.fund_class,
                            Some(Class::Acoes),
                            "Ações",
                            self.channel.0.clone(),
                            &self.query,
                        );
                        handle_selectable_value(
                            ui,
                            &mut self.fund_class,
                            Some(Class::RendaFixa),
                            "Renda Fixa",
                            self.channel.0.clone(),
                            &self.query,
                        );
                        handle_selectable_value(
                            ui,
                            &mut self.fund_class,
                            Some(Class::Cambial),
                            "Cambial",
                            self.channel.0.clone(),
                            &self.query,
                        );
                        handle_selectable_value(
                            ui,
                            &mut self.fund_class,
                            Some(Class::MultiMarket),
                            "MultiMercado",
                            self.channel.0.clone(),
                            &self.query,
                        );
                    });

                    ui.add_space(5.0);
                    let nr_rows = self.result_funds.height();
                    let cols: Vec<&str> = vec!["CNPJ_FUNDO", "DENOM_SOCIAL"];

                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        TableBuilder::new(ui)
                            //.column(Column::auto().at_most(20.0))
                            .column(Column::auto().at_least(40.0).resizable(false))
                            .column(Column::remainder().at_most(40.0))
                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                            .striped(true)
                            .resizable(false)
                            .header(20.0, |mut header| {
                                header.col(|ui| {
                                    ui.label("cnpj");
                                });
                                header.col(|ui| {
                                    ui.label("nome");
                                });
                            })
                            .body(|body| {
                                body.rows(20.0, nr_rows, |mut row| {
                                    let row_index = row.index();

                                    for col in &cols {
                                        row.col(|ui| {
                                            if let Ok(column) = self.result_funds.column(col) {
                                                if let Ok(value) = column.get(row_index) {
                                                    if let Some(value_str) = value.get_str() {
                                                        if col.contains("CNPJ_FUNDO") {
                                                            if ui.link(value_str).clicked() {
                                                                let strcnpj = value_str.to_string();
                                                                let _ = self.channel.0.send(
                                                                    Message::NewTab(
                                                                        strcnpj.clone(),
                                                                    ),
                                                                );
                                                            }
                                                        } else {
                                                            ui.label(value_str);
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                    }
                                });
                            });
                    });
                });

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
    }
}

fn handle_selectable_value(
    ui: &mut egui::Ui,
    fund_class: &mut Option<Class>,
    class: Option<Class>,
    label: &str,
    channel: tokio::sync::mpsc::UnboundedSender<Message>,
    query: &str,
) {
    if ui
        .selectable_value(fund_class, class.clone(), label)
        .clicked()
    {
        send_find_message(channel, query, class.clone());
    }
}

fn send_find_message(
    channel: tokio::sync::mpsc::UnboundedSender<Message>,
    query: &str,
    class: Option<Class>,
) {
    let sender = channel.clone();
    let text = query.to_string();
    let class = class.clone();
    tokio::spawn(async move {
        let _ = sender.send(Message::SearchFunds(text, class));
    });
}
