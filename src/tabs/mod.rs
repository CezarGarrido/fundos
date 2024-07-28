use egui::{Ui, WidgetText};
pub mod fund_tab;
pub mod home_tab;
use crate::tabs::fund_tab::FundTab;
use egui_dock::{NodeIndex, SurfaceIndex};
use home_tab::HomeTab;

pub trait Tab {
    fn title(&self) -> WidgetText;
    fn ui(&mut self, ui: &mut Ui);
    fn closeable(&self) -> bool;
}

// Adicione outro tipo de tab
pub enum TabType {
    Fund(FundTab),
    Home(HomeTab),
}

impl Tab for TabType {
    fn title(&self) -> WidgetText {
        match self {
            TabType::Fund(tab) => tab.title(),
            TabType::Home(tab) => tab.title(),
            // Adicione outros tipos de tabs aqui
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.push_id(format!("{}_", self.title().text()), |ui| {
            egui::Frame::none()
                //.fill(ui.style().visuals.extreme_bg_color)
                .inner_margin(-2.0)
                .outer_margin(0.0)
                .show(ui, |ui| {
                    match self {
                        TabType::Fund(tab) => tab.ui(ui),
                        TabType::Home(tab) => tab.ui(ui),
                        // Adicione outros tipos de tabs aqui
                    }
                });
        });
    }

    fn closeable(&self) -> bool {
        match self {
            TabType::Fund(tab) => tab.closeable(),
            TabType::Home(tab) => tab.closeable(),
            // Adicione outros tipos de tabs aqui
        }
    }
}

pub struct TabViewer {
    pub open_window: bool,
}

impl egui_dock::TabViewer for TabViewer {
    type Tab = TabType;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        tab.title()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        tab.ui(ui);
    }

    fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
        tab.closeable()
    }

    fn on_add(&mut self, _surface: SurfaceIndex, _node: NodeIndex) {
        self.open_window = true;
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }
}
