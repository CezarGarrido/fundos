//! Fundos Frontend — Thin egui client that consumes the backend API.
//!
//! All heavy data work happens server-side. The frontend calls api_client and renders JSON.

use egui_dock::{DockArea, DockState, NodeIndex, Style};

use crate::{
    ui::{
        modal::about::About,
        tabs::{self, TabType, TabViewer},
    },
    util,
};

enum AppAction {
    OpenSearch,
    OpenDashboard,
    OpenAtivos,
}

pub struct TemplateApp {
    tab_viewer: TabViewer,
    tree: DockState<TabType>,

    // Search state
    search_modal: crate::ui::fund::modal::search::Search,

    // Status bar
    pub status: String,
    pub open_logs: bool,

    about_modal: About,
    current_theme_dark: Option<bool>,
}

impl Default for TemplateApp {
    fn default() -> Self {
        let tree: DockState<TabType> =
            DockState::new(vec![TabType::Home(tabs::home_tab::HomeTab::new(
                "Início".to_string(),
            ))]);
        let tab_viewer = TabViewer { open_window: false };

        Self {
            tree,
            tab_viewer,
            search_modal: crate::ui::fund::modal::search::Search::new(false),
            status: String::new(),
            open_logs: false,
            about_modal: About::new(),
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
        cc.egui_ctx.set_visuals(get_premium_visuals(false));
        Self {
            current_theme_dark: Some(false),
            ..Default::default()
        }
    }

    // Inline search methods removed

    fn open_fund_tab(&mut self, cnpj: String) {
        let found_idx: Option<usize> = self
            .tree
            .iter_all_tabs()
            .enumerate()
            .find_map(|(i, (_node, tab))| {
                if let TabType::Fund(ft) = tab {
                    if ft.cnpj == cnpj { Some(i) } else { None }
                } else {
                    None
                }
            });
        if let Some(index) = found_idx {
            let main_surface = self.tree.main_surface_mut();
            let _ = main_surface.set_active_tab(NodeIndex(0), egui_dock::TabIndex(index));
        } else {
            let fund_tab = crate::ui::fund::tab::FundTab::new(cnpj);
            let main_surface = self.tree.main_surface_mut();
            main_surface.push_to_focused_leaf(TabType::Fund(fund_tab));
        }
    }

    fn process_home_actions(&mut self, ctx: &egui::Context) {
        let indices: Vec<usize> = self
            .tree
            .iter_all_tabs()
            .enumerate()
            .filter_map(|(i, (_node, tab))| {
                if let TabType::Home(ht) = tab {
                    if ht.action.is_some() { Some(i) } else { None }
                } else {
                    None
                }
            })
            .collect();

        for idx in indices {
            let action = if let Some((_i, (_path, TabType::Home(ht)))) =
                self.tree.iter_all_tabs_mut().enumerate().find(|(i, _)| *i == idx)
            {
                ht.take_action()
            } else {
                None
            };

            if let Some(action) = action {
                match action {
                    tabs::home_tab::HomeAction::OpenSearch => {
                        self.search_modal.open(true, ctx);
                    }
                    tabs::home_tab::HomeAction::OpenDashboard => {
                        let db_tab = crate::ui::fund::tab::dashboard::DashboardTab::new(
                            "Dashboard".to_string(),
                        );
                        let main_surface = self.tree.main_surface_mut();
                        main_surface.push_to_focused_leaf(TabType::Dashboard(db_tab));
                    }
                    tabs::home_tab::HomeAction::OpenAtivos => {
                        let ativos_tab = crate::ui::fund::tab::ativos::AssetsMarketTab::new();
                        let main_surface = self.tree.main_surface_mut();
                        main_surface.push_to_focused_leaf(TabType::Ativos(ativos_tab));
                    }
                }
            }
        }
    }
}

impl eframe::App for TemplateApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut action: Option<AppAction> = None;
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::F) {
                if !self.search_modal.open_window {
                    action = Some(AppAction::OpenSearch);
                }
            }
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::D) {
                action = Some(AppAction::OpenDashboard);
            }
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::A) {
                action = Some(AppAction::OpenAtivos);
            }
        });
        match action {
            Some(AppAction::OpenSearch) => self.search_modal.open(true, ctx),
            Some(AppAction::OpenDashboard) => {
                let db_tab = crate::ui::fund::tab::dashboard::DashboardTab::new(
                    "Dashboard".to_string(),
                );
                let main_surface = self.tree.main_surface_mut();
                main_surface.push_to_focused_leaf(TabType::Dashboard(db_tab));
            }
            Some(AppAction::OpenAtivos) => {
                let ativos_tab = crate::ui::fund::tab::ativos::AssetsMarketTab::new();
                let main_surface = self.tree.main_surface_mut();
                main_surface.push_to_focused_leaf(TabType::Ativos(ativos_tab));
            }
            None => {}
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx();
        let current_dark = ctx.theme() == egui::Theme::Dark;
        if self.current_theme_dark != Some(current_dark) {
            self.current_theme_dark = Some(current_dark);
            ctx.set_visuals(get_premium_visuals(current_dark));
        }

        egui::TopBottomPanel::top("top_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("\u{2630}", |ui| {
                    if ui.button("Buscar Fundos (Ctrl+F)").clicked() {
                        self.search_modal.open(true, ui.ctx());
                        ui.close_menu();
                    }
                    if ui.button("Dashboard").clicked() {
                        let db_tab = crate::ui::fund::tab::dashboard::DashboardTab::new(
                            "Dashboard".to_string(),
                        );
                        let main_surface = self.tree.main_surface_mut();
                        main_surface.push_to_focused_leaf(TabType::Dashboard(db_tab));
                        ui.close_menu();
                    }
                    if ui.button("Ativos do Mercado").clicked() {
                        let ativos_tab = crate::ui::fund::tab::ativos::AssetsMarketTab::new();
                        let main_surface = self.tree.main_surface_mut();
                        main_surface.push_to_focused_leaf(TabType::Ativos(ativos_tab));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Sobre").clicked() {
                        self.about_modal.open(true);
                        ui.close_menu();
                    }
                });
                ui.heading("Fundos");
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            DockArea::new(&mut self.tree)
                .style(Style::from_egui(ui.style().as_ref()))
                .show_add_buttons(false)
                .show_add_popup(false)
                .show_inside(ui, &mut self.tab_viewer);
        });

        // Process any pending actions from home tab
        self.process_home_actions(ui.ctx());

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if !self.status.is_empty() {
                    ui.label(&self.status);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn = ui
                        .small_button(egui_phosphor::regular::WARNING)
                        .on_hover_ui(|ui| {
                            ui.label("Log do Sistema");
                        });
                    if btn.clicked() {
                        self.open_logs = !self.open_logs;
                    }
                    egui::global_theme_preference_switch(ui);
                });
            });
        });

        util::toaster().show(ui);

        // Logger window
        if self.open_logs {
            egui::Window::new("Log do Sistema")
                .open(&mut self.open_logs)
                .show(ui.ctx(), |ui| {
                    egui_logger::logger_ui().show(ui);
                });
        }

        // Search modal
        if self.search_modal.open_window {
            if let Some(cnpj) = self.search_modal.show(ui) {
                self.open_fund_tab(cnpj);
                self.search_modal.open(false, ui.ctx());
            }
        }

        if self.about_modal.show {
            self.about_modal.show(ui);
        }
    }
}

fn get_premium_visuals(dark_mode: bool) -> egui::Visuals {
    if dark_mode {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(18, 19, 23);
        visuals.window_fill = egui::Color32::from_rgb(26, 27, 32);
        visuals.faint_bg_color = egui::Color32::from_rgb(26, 27, 32);
        visuals.extreme_bg_color = egui::Color32::from_rgb(13, 14, 16);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(24, 25, 29);
        visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(18, 19, 23);
        visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(226, 232, 240);
        visuals.widgets.noninteractive.bg_stroke.color = egui::Color32::from_rgb(38, 41, 49);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(33, 35, 41);
        visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(203, 213, 224);
        visuals.widgets.inactive.bg_stroke.color = egui::Color32::from_rgb(45, 48, 56);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(45, 48, 56);
        visuals.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
        visuals.widgets.hovered.bg_stroke.color = egui::Color32::from_rgb(59, 130, 246);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(59, 130, 246);
        visuals.widgets.active.fg_stroke.color = egui::Color32::WHITE;
        visuals.selection.bg_fill = egui::Color32::from_rgb(59, 130, 246);
        visuals.selection.stroke.color = egui::Color32::WHITE;
        visuals
    } else {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = egui::Color32::from_rgb(255, 255, 255);
        visuals.window_fill = egui::Color32::from_rgb(255, 255, 255);
        visuals.faint_bg_color = egui::Color32::from_rgb(245, 246, 248);
        visuals.extreme_bg_color = egui::Color32::from_rgb(248, 249, 250);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(245, 246, 248);
        visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(250, 251, 252);
        visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(44, 55, 72);
        visuals.widgets.noninteractive.bg_stroke.color = egui::Color32::from_rgb(222, 226, 230);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(248, 249, 250);
        visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(74, 85, 104);
        visuals.widgets.inactive.bg_stroke.color = egui::Color32::from_rgb(226, 232, 240);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(237, 242, 247);
        visuals.widgets.hovered.fg_stroke.color = egui::Color32::from_rgb(26, 32, 44);
        visuals.widgets.hovered.bg_stroke.color = egui::Color32::from_rgb(26, 115, 232);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(226, 232, 240);
        visuals.widgets.active.fg_stroke.color = egui::Color32::from_rgb(26, 32, 44);
        visuals.selection.bg_fill = egui::Color32::from_rgb(26, 115, 232);
        visuals.selection.stroke.color = egui::Color32::WHITE;
        visuals
    }
}
