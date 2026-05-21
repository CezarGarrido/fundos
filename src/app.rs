use crate::{provider::cvm::fund, util};

use chrono::{Datelike, NaiveDate};
use egui::FontId;
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabAddAlign};
use egui_toast::{Toast, ToastOptions};
use polars::frame::DataFrame;
use polars::prelude::NamedFrom;
use std::collections::HashMap;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::time::{timeout, Duration};
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
            tab::{ativos::AssetsMarketTab, dashboard::DashboardTab, FundTab},
        },
        modal::about::About,
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
    #[serde(skip)]
    current_theme_dark: Option<bool>,
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
            current_theme_dark: None,
        }
    }
}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();

        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.set_theme(egui::Theme::Light);

        cc.egui_ctx.options_mut(|opt| {
            opt.warn_on_id_clash = false;
        });

        let app = Self {
            current_theme_dark: Some(false),
            ..Default::default()
        };
        cc.egui_ctx.set_visuals(get_premium_visuals(false));

        app
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
            let _ = main_surface.set_active_tab(NodeIndex(0), egui_dock::TabIndex(index));
        } else {
            let main_surface = self.tree.main_surface_mut();
            main_surface.set_focused_node(egui_dock::NodeIndex(2));
            let new_fund_tab = FundTab::new(cnpj.clone(), df.clone(), self.channel.0.clone());
            main_surface.push_to_focused_leaf(TabType::Fund(new_fund_tab));
        }

        if df.height() > 0 {
            if let Ok(s) = df.column("DENOM_SOCIAL") {
                if let Ok(value) = s.get(0) {
                    if let Some(name) = value.get_str() {
                        self.history.add(cnpj.clone(), name.to_string());
                        let _ = self.history.save();
                        let _ = self.history.load();
                    }
                }
            }
        }
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
            let _ = main_surface.set_active_tab(NodeIndex(0), egui_dock::TabIndex(index));
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

    pub fn add_ativos_tab(&mut self) {
        let tabs: Vec<_> = self
            .tree
            .iter_all_tabs()
            .map(|(_, tab)| tab.to_owned())
            .collect();

        if let Some(index) = tabs
            .iter()
            .position(|tb| tb.title().text().contains("Ativos"))
        {
            let main_surface = self.tree.main_surface_mut();
            let _ = main_surface.set_active_tab(NodeIndex(0), egui_dock::TabIndex(index));
        } else {
            let main_surface = self.tree.main_surface_mut();
            main_surface.set_focused_node(egui_dock::NodeIndex(2));
            let ativos_tab = AssetsMarketTab::new(self.channel.0.clone());
            main_surface.push_to_focused_leaf(TabType::Ativos(ativos_tab));
        }
    }

    fn handle_update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let ctxc = ctx.clone();
        let sender = self.channel.0.clone();

        // Atalhos globais de teclado
        ctx.input_mut(|i| {
            // Ctrl+F → Pesquisar Fundos
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::F) {
                let _ = sender.send(Message::OpenSearchWindow(true));
            }
            // Ctrl+D → Painel Geral
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::D) {
                let _ = sender.send(Message::OpenDashboardTab);
            }
            // Ctrl+A → Ativos do Mercado
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::A) {
                use chrono::{Duration, Local, Months};
                let end = Local::now().naive_local().date();
                let start = end
                    .checked_sub_months(Months::new(6))
                    .unwrap_or(end - Duration::days(183));
                let _ = sender.send(Message::OpenAtivosTab(start, end));
            }
        });

        if let Ok(message) = self.channel.1.try_recv() {
            match message {
                Message::OpenSearchWindow(value) => {
                    self.search.set_loading(true);
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
                        if let Err(_cache_err) =
                            handle_fund_data(cnpj.clone(), true, r.clone(), &sender, &ctxc).await
                        {
                            util::toaster().add(Toast {
                                kind: egui_toast::ToastKind::Info,
                                text: format!(
                                    "CNPJ {} não cadastrado localmente. Baixando dados online...",
                                    cnpj
                                )
                                .into(),
                                options: ToastOptions::default().duration_in_seconds(4.0),
                                ..Default::default()
                            });
                            ctxc.request_repaint();

                            if let Err(online_err) =
                                handle_fund_data(cnpj.clone(), false, r.clone(), &sender, &ctxc)
                                    .await
                            {
                                log::error!(
                                    "Erro ao obter dados online do fundo {}: {}",
                                    cnpj,
                                    online_err
                                );
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Error,
                                    text: format!(
                                        "Erro ao obter dados online do fundo (CNPJ: {})",
                                        cnpj
                                    )
                                    .into(),
                                    options: ToastOptions::default().duration_in_seconds(4.0),
                                    ..Default::default()
                                });
                            }
                        }
                    });
                }

                Message::Profit(cnpj, start_date, end_date) => {
                    let informe = self.informe.clone();
                    let sender_clone = sender.clone();
                    let ctx_clone = ctxc.clone();

                    tokio::spawn(async move {
                        let cdi_future = timeout(
                            Duration::from_secs(15),
                            indices::cdi::async_dataframe(start_date, end_date),
                        );
                        let ibov_future = timeout(
                            Duration::from_secs(15),
                            indices::ibovespa::async_dataframe(start_date, end_date),
                        );
                        let profitability_future = timeout(
                            Duration::from_secs(15),
                            informe.async_profit(cnpj.clone(), start_date, end_date),
                        );

                        let (profitability_result, cdi_result, ibov_result) =
                            tokio::join!(profitability_future, cdi_future, ibov_future);

                        let cdi_dataframe = match cdi_result {
                            Ok(Ok(df)) => df,
                            Ok(Err(e)) => {
                                log::warn!("Dados do CDI indisponíveis: {}", e);
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Warning,
                                    text: "Dados históricos do CDI indisponíveis para o período."
                                        .into(),
                                    options: ToastOptions::default().duration_in_seconds(4.0),
                                    ..Default::default()
                                });
                                DataFrame::empty()
                            }
                            Err(_) => {
                                log::error!("Timeout ao obter dados do CDI");
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Warning,
                                    text: "Tempo limite atingido ao carregar dados do CDI.".into(),
                                    options: ToastOptions::default().duration_in_seconds(4.0),
                                    ..Default::default()
                                });
                                DataFrame::empty()
                            }
                        };

                        let profitability_dataframe = match profitability_result {
                            Ok(Ok(df)) => df,
                            Ok(Err(e)) => {
                                log::warn!(
                                    "Rentabilidade indisponível para o CNPJ {}: {}",
                                    cnpj,
                                    e
                                );
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Warning,
                                    text: format!(
                                        "Rentabilidade do fundo indisponível no período (CNPJ: {})",
                                        cnpj
                                    )
                                    .into(),
                                    options: ToastOptions::default().duration_in_seconds(4.0),
                                    ..Default::default()
                                });
                                DataFrame::empty()
                            }
                            Err(_) => {
                                log::error!(
                                    "Timeout ao obter rentabilidade do fundo (CNPJ: {})",
                                    cnpj
                                );
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Warning,
                                    text: format!("Tempo limite atingido ao carregar rentabilidade do fundo (CNPJ: {})", cnpj).into(),
                                    options: ToastOptions::default().duration_in_seconds(4.0),
                                    ..Default::default()
                                });
                                DataFrame::empty()
                            }
                        };

                        let ibov_dataframe = match ibov_result {
                            Ok(Ok(df)) => df,
                            Ok(Err(e)) => {
                                log::warn!("Dados do IBOV indisponíveis: {}", e);
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Warning,
                                    text:
                                        "Dados históricos do IBOVESPA indisponíveis para o período."
                                            .into(),
                                    options: ToastOptions::default().duration_in_seconds(4.0),
                                    ..Default::default()
                                });
                                DataFrame::empty()
                            }
                            Err(_) => {
                                log::error!("Timeout ao obter dados do IBOV");
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Warning,
                                    text: "Tempo limite atingido ao carregar dados do IBOVESPA."
                                        .into(),
                                    options: ToastOptions::default().duration_in_seconds(4.0),
                                    ..Default::default()
                                });
                                DataFrame::empty()
                            }
                        };

                        let _ = sender_clone.send(Message::ProfitResult(
                            cnpj.clone(),
                            profitability_dataframe,
                            cdi_dataframe,
                            ibov_dataframe,
                        ));
                        ctx_clone.request_repaint();
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
                    let sender_clone = sender.clone();
                    let ctx_clone = ctxc.clone();

                    tokio::spawn(async move {
                        let mut current_year: i32 = year.parse().unwrap_or(2026);
                        let mut current_month: u32 = month.parse().unwrap_or(5);

                        let mut pl = DataFrame::empty();
                        let mut assets = DataFrame::empty();
                        let mut top_assets = DataFrame::empty();
                        let mut loaded_year = year.clone();
                        let mut loaded_month = month.clone();
                        let mut success = false;

                        for _ in 0..6 {
                            let y_str = current_year.to_string();
                            let m_str = format!("{:02}", current_month);

                            // Envelopa a chamada assíncrona com timeout de 15 segundos
                            let result = timeout(
                                Duration::from_secs(15),
                                portfolio.async_assets(
                                    cnpj.clone(),
                                    y_str.clone(),
                                    m_str.clone(),
                                    true,
                                ),
                            )
                            .await;

                            if let Ok(Ok((res_pl, res_assets, res_top_assets))) = result {
                                if !res_assets.is_empty() {
                                    pl = res_pl;
                                    assets = res_assets;
                                    top_assets = res_top_assets;
                                    loaded_year = y_str;
                                    loaded_month = m_str;
                                    success = true;
                                    break;
                                }
                            }

                            // Subtrai um mês
                            if current_month == 1 {
                                current_month = 12;
                                current_year -= 1;
                            } else {
                                current_month -= 1;
                            }
                        }

                        if success {
                            let m_num: i32 = loaded_month.parse().unwrap_or(1);
                            let m_name = match m_num {
                                1 => "Janeiro",
                                2 => "Fevereiro",
                                3 => "Março",
                                4 => "Abril",
                                5 => "Maio",
                                6 => "Junho",
                                7 => "Julho",
                                8 => "Agosto",
                                9 => "Setembro",
                                10 => "Outubro",
                                11 => "Novembro",
                                12 => "Dezembro",
                                _ => "Janeiro",
                            };
                            util::toaster().add(Toast {
                                kind: egui_toast::ToastKind::Success,
                                text: format!(
                                    "Carteira mais recente disponível carregada (Ref: {}/{})",
                                    m_name, loaded_year
                                )
                                .into(),
                                options: ToastOptions::default().duration_in_seconds(4.0),
                                ..Default::default()
                            });
                        } else {
                            util::toaster().add(Toast {
                                kind: egui_toast::ToastKind::Warning,
                                text: format!(
                                    "Nenhuma carteira encontrada nos últimos 6 meses (CNPJ: {})",
                                    cnpj
                                )
                                .into(),
                                options: ToastOptions::default().duration_in_seconds(4.0),
                                ..Default::default()
                            });
                        }

                        let _ = sender_clone.send(Message::AssetsResult(
                            cnpj.clone(),
                            assets,
                            top_assets,
                            pl,
                            loaded_year,
                            loaded_month,
                        ));
                        ctx_clone.request_repaint();
                    });
                }

                Message::AssetsResult(
                    cnpj,
                    assets,
                    top_assets,
                    patrimonio_liquido,
                    loaded_year,
                    loaded_month,
                ) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Fund(tab) = tb {
                            if *tab.title().text().to_string() == cnpj {
                                tab.set_assets_dataframe(assets.clone());
                                tab.set_top_assets_dataframe(top_assets.clone());
                                tab.set_pl_dataframe(patrimonio_liquido.clone());

                                // Sincronizar o estado da data selecionada na interface com a carteira encontrada
                                tab.portfolio_ui.filter_year = loaded_year.clone();
                                tab.portfolio_ui.filter_month = loaded_month.clone();

                                let m_num: i32 = loaded_month.parse().unwrap_or(1);
                                let m_name = match m_num {
                                    1 => "Janeiro",
                                    2 => "Fevereiro",
                                    3 => "Março",
                                    4 => "Abril",
                                    5 => "Maio",
                                    6 => "Junho",
                                    7 => "Julho",
                                    8 => "Agosto",
                                    9 => "Setembro",
                                    10 => "Outubro",
                                    11 => "Novembro",
                                    12 => "Dezembro",
                                    _ => "Janeiro",
                                };
                                tab.portfolio_ui.filter_date =
                                    format!("{}/{}", m_name, loaded_year);

                                tab.set_assets_loading(false);
                                ctxc.request_repaint();
                                break;
                            }
                        }
                    }
                }
                Message::ResultFunds(df) => {
                    self.search.set_loading(false);
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
                                let _ = sender.send(Message::ResultFunds(DataFrame::empty()));
                                ctxc.request_repaint();
                                log::error!("Erro ao buscar fundos {:?}", err);
                                util::toaster().add(Toast {
                                    kind: egui_toast::ToastKind::Error,
                                    text: "Erro ao buscar fundos".into(),
                                    options: ToastOptions::default().duration_in_seconds(3.0),
                                    ..Default::default()
                                });
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
                Message::OpenAtivosTab(start, end) => {
                    self.add_ativos_tab();
                    let portfolio = self.portfolio.clone();
                    let s = sender.clone();
                    tokio::spawn(async move {
                        match portfolio.async_market_assets(start, end).await {
                            Ok(df) => {
                                let _ = s.send(Message::AtivosTabResult(df));
                                ctxc.request_repaint();
                            }
                            Err(err) => {
                                log::error!("Erro ao calcular ativos de mercado: {}", err);
                            }
                        }
                    });
                }
                Message::AtivosTabResult(df) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Ativos(tab) = tb {
                            tab.set_data(df.clone());
                            ctxc.request_repaint();
                            break;
                        }
                    }
                }
                Message::OpenHistoricoTab(cnpj, start, end) => {
                    let portfolio = self.portfolio.clone();
                    let s = sender.clone();
                    tokio::spawn(async move {
                        match portfolio
                            .async_historical_assets(cnpj.clone(), start, end)
                            .await
                        {
                            Ok(df) => {
                                let _ = s.send(Message::HistoricoTabResult(cnpj, df));
                                ctxc.request_repaint();
                            }
                            Err(err) => {
                                log::error!("Erro ao carregar histórico: {}", err);
                            }
                        }
                    });
                }
                Message::HistoricoTabResult(cnpj, df) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Historico(tab) = tb {
                            if tab.cnpj == cnpj {
                                tab.set_data(df.clone());
                                ctxc.request_repaint();
                                break;
                            }
                        }
                        // Also check inside FundTab's historico_tab
                        if let TabType::Fund(tab) = tb {
                            if let Some(ref mut htab) = tab.historico_tab {
                                if htab.cnpj == cnpj {
                                    htab.set_data(df.clone());
                                }
                            }
                            if tab.portfolio_ui.cnpj == cnpj {
                                tab.portfolio_ui.fund_history = Some(df.clone());
                            }
                            ctxc.request_repaint();
                        }
                    }
                }
                Message::FetchYahooPrice(codigo, _cnpj, start, end) => {
                    let s = sender.clone();
                    tokio::spawn(async move {
                        let ticker = format!("{}.SA", codigo);
                        match yahoo_finance_api::YahooConnector::new() {
                            Ok(provider) => {
                                let start_ts =
                                    yahoo_finance_api::time::OffsetDateTime::from_unix_timestamp(
                                        NaiveDate::from_ymd_opt(
                                            start.year(),
                                            start.month(),
                                            start.day(),
                                        )
                                        .unwrap()
                                        .and_hms_opt(0, 0, 0)
                                        .unwrap()
                                        .and_utc()
                                        .timestamp(),
                                    )
                                    .unwrap();
                                let end_ts =
                                    yahoo_finance_api::time::OffsetDateTime::from_unix_timestamp(
                                        NaiveDate::from_ymd_opt(end.year(), end.month(), end.day())
                                            .unwrap()
                                            .and_hms_opt(23, 59, 59)
                                            .unwrap()
                                            .and_utc()
                                            .timestamp(),
                                    )
                                    .unwrap();
                                match provider.get_quote_history(&ticker, start_ts, end_ts).await {
                                    Ok(resp) => {
                                        if let Ok(quotes) = resp.quotes() {
                                            let prices: Vec<f64> =
                                                quotes.iter().map(|q| q.adjclose).collect();
                                            let dates: Vec<String> = quotes
                                                .iter()
                                                .map(|q| {
                                                    chrono::DateTime::from_timestamp(
                                                        q.timestamp as i64,
                                                        0,
                                                    )
                                                    .unwrap()
                                                    .naive_utc()
                                                    .format("%Y-%m-%d")
                                                    .to_string()
                                                })
                                                .collect();
                                            let df = polars::frame::DataFrame::new(vec![
                                                polars::prelude::Series::new("date", dates),
                                                polars::prelude::Series::new("adjclose", prices),
                                            ])
                                            .unwrap_or_default();
                                            let _ = s.send(Message::YahooPriceResult(codigo, df));
                                            ctxc.request_repaint();
                                        }
                                    }
                                    Err(err) => {
                                        log::error!("Yahoo price error for {}: {}", ticker, err);
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("YahooConnector error: {}", err);
                            }
                        }
                    });
                }
                Message::YahooPriceResult(codigo, df) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Fund(tab) = tb {
                            // Route to historico tab inside FundTab
                            if let Some(ref mut htab) = tab.historico_tab {
                                htab.set_yahoo_prices(codigo.clone(), df.clone());
                                ctxc.request_repaint();
                            }
                            // Route to portfolio modal inside FundTab
                            tab.portfolio_ui
                                .asset_modal
                                .yahoo_prices
                                .insert(codigo.clone(), df.clone());
                            tab.portfolio_ui.asset_modal.yahoo_loading = false;
                            ctxc.request_repaint();
                        }
                        if let TabType::Historico(tab) = tb {
                            tab.set_yahoo_prices(codigo.clone(), df.clone());
                            ctxc.request_repaint();
                        }
                        if let TabType::Ativos(tab) = tb {
                            tab.asset_modal
                                .yahoo_prices
                                .insert(codigo.clone(), df.clone());
                            tab.asset_modal.yahoo_loading = false;
                            ctxc.request_repaint();
                        }
                    }
                }
                Message::FetchAssetHolders(asset_id, start_date, end_date) => {
                    let portfolio = self.portfolio.clone();
                    let s = sender.clone();
                    tokio::spawn(async move {
                        match portfolio
                            .async_asset_holders(asset_id.clone(), start_date, end_date)
                            .await
                        {
                            Ok(df) => {
                                let _ = s.send(Message::AssetHoldersResult(asset_id, df));
                                ctxc.request_repaint();
                            }
                            Err(e) => {
                                log::error!("Erro FetchAssetHolders: {}", e);
                            }
                        }
                    });
                }
                Message::AssetHoldersResult(_asset_id, df) => {
                    let tabs: Vec<_> = self.tree.iter_all_tabs_mut().map(|(_, tab)| tab).collect();
                    for tb in tabs {
                        if let TabType::Ativos(tab) = tb {
                            tab.asset_modal.holders_data = Some(df.clone());
                            tab.asset_modal.holders_loading = false;
                            ctxc.request_repaint();
                            break;
                        }
                    }
                }
            }
        }
    }

    // Função para configurar o painel superior
    fn setup_top_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top_panel").show_inside(ui, |ui| {
            self.setup_menu_bar(ui);
        });
    }

    // Função para configurar a barra de menu
    fn setup_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            let font_id = FontId::proportional(16.0);
            let icon = egui::RichText::new(egui_phosphor::regular::LIST.to_string()).font(font_id);
            ui.menu_button(icon, |ui| {
                self.setup_fund_menu(ui);
                if ui.button("Sobre").clicked() {
                    self.about_modal.open(true);
                }
                ui.separator();
                if ui.button("Sair").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
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
    fn setup_central_panel(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if !self.search.open_window {
                self.search.set_result(DataFrame::empty());
            }
            self.search.show(ui);
            self.asset_detail_modal.show(ui);
            self.about_modal.show(ui);
            self.setup_dock_area(ui);
        });
    }

    // Função para configurar a Dock Area
    fn setup_dock_area(&mut self, ui: &mut egui::Ui) {
        let has_home = self
            .tree
            .iter_all_tabs()
            .any(|(_, tab)| matches!(tab, TabType::Home(_)));

        if !has_home {
            let home_tab = TabType::Home(HomeTab::new(
                "Início".to_string(),
                self.channel.0.clone(),
                self.history.clone(),
            ));

            if self.tree.iter_all_tabs().count() == 0 {
                self.tree = DockState::new(vec![home_tab]);
            } else {
                let main_surface = self.tree.main_surface_mut();
                main_surface.push_to_focused_leaf(home_tab);
            }
        }

        egui::Frame::NONE.inner_margin(5.0).show(ui, |ui| {
            DockArea::new(&mut self.tree)
                .style({
                    let mut style = Style::from_egui(ui.style());
                    style.buttons.add_tab_align = TabAddAlign::Left;
                    style
                })
                .show_add_buttons(true)
                .show_inside(ui, &mut self.tab_viewer);
        });
    }
}

impl eframe::App for TemplateApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx();

        // Reativamente aplica e monitora o tema selecionado
        let current_dark = ctx.theme() == egui::Theme::Dark;
        if self.current_theme_dark != Some(current_dark) {
            self.current_theme_dark = Some(current_dark);
            ctx.set_visuals(get_premium_visuals(current_dark));
        }

        self.handle_update(ctx, frame);
        self.setup_top_panel(ui);
        self.show_statusbar(ui, frame);
        self.setup_central_panel(ui);
        util::toaster().show(ui);
    }
}

async fn handle_fund_data(
    cnpj: String,
    use_cache: bool,
    r: Register,
    sender: &UnboundedSender<Message>,
    ctxc: &egui::Context,
) -> Result<(), fund::Error> {
    let res = r.async_find_by_cnpj(cnpj.clone(), use_cache).await;
    match res {
        Ok(fund_dataframe) => {
            let _ = sender.send(Message::OpenTab(cnpj, fund_dataframe));
            ctxc.request_repaint();
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn get_premium_visuals(dark_mode: bool) -> egui::Visuals {
    if dark_mode {
        let mut visuals = egui::Visuals::dark();

        // Cores premium da paleta moderna (Dark Mode)
        visuals.panel_fill = egui::Color32::from_rgb(18, 19, 23); // Fundo escuro profundo premium (Slate)
        visuals.window_fill = egui::Color32::from_rgb(26, 27, 32); // Cinza escuro para janelas/modais
        visuals.faint_bg_color = egui::Color32::from_rgb(26, 27, 32); // Listras de tabelas e separadores
        visuals.extreme_bg_color = egui::Color32::from_rgb(13, 14, 16); // Caixa de texto escura

        // Cores de bordas e elementos não-interativos
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(24, 25, 29);
        visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(18, 19, 23);
        visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(226, 232, 240); // Texto claro
        visuals.widgets.noninteractive.bg_stroke.color = egui::Color32::from_rgb(38, 41, 49); // Bordas super sutis e finas

        // Botões (Inativos/Normais)
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(33, 35, 41);
        visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(203, 213, 224);
        visuals.widgets.inactive.bg_stroke.color = egui::Color32::from_rgb(45, 48, 56);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);

        // Botões (Hovered / Selecionados)
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(45, 48, 56);
        visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
        visuals.widgets.hovered.bg_stroke.color = egui::Color32::from_rgb(59, 130, 246);

        // Botões (Active / Clicados)
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(59, 130, 246);
        visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;

        // Destaque ativo (Azul elétrico premium)
        visuals.selection.bg_fill = egui::Color32::from_rgb(59, 130, 246);
        visuals.selection.stroke.color = egui::Color32::WHITE;

        visuals
    } else {
        let mut visuals = egui::Visuals::light();

        // Cores premium da paleta moderna (Light Mode: Branco Puro + Cinza Claro)
        visuals.panel_fill = egui::Color32::from_rgb(255, 255, 255); // Branco puro para abas e painéis principais
        visuals.window_fill = egui::Color32::from_rgb(255, 255, 255); // Branco puro para janelas/modais
        visuals.faint_bg_color = egui::Color32::from_rgb(245, 246, 248); // Cinza muito claro para listras de tabelas/zebra e separadores
        visuals.extreme_bg_color = egui::Color32::from_rgb(248, 249, 250); // Caixa de texto muito limpa

        // Cores de bordas e elementos não-interativos
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(245, 246, 248); // Fundo cinza suave secundário
        visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(250, 251, 252);
        visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(44, 55, 72); // Texto cinza escuro/azul elegante
        visuals.widgets.noninteractive.bg_stroke.color = egui::Color32::from_rgb(222, 226, 230); // Bordas super sutis e finas

        // Botões (Inativos/Normais)
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(248, 249, 250);
        visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(74, 85, 104);
        visuals.widgets.inactive.bg_stroke.color = egui::Color32::from_rgb(226, 232, 240);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);

        // Botões (Hovered / Selecionados)
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(237, 242, 247);
        visuals.widgets.hovered.fg_stroke.color = egui::Color32::from_rgb(26, 32, 44);
        visuals.widgets.hovered.bg_stroke.color = egui::Color32::from_rgb(26, 115, 232); // Bordas azuis no hover

        // Botões (Active / Clicados)
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(226, 232, 240);
        visuals.widgets.active.fg_stroke.color = egui::Color32::from_rgb(26, 32, 44);

        // Destaque ativo (Azul corporativo limpo)
        visuals.selection.bg_fill = egui::Color32::from_rgb(26, 115, 232);
        visuals.selection.stroke.color = egui::Color32::WHITE;

        visuals
    }
}
