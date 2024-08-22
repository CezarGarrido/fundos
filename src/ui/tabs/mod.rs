use egui::{Ui, WidgetText};
pub mod config_tab;
pub mod home_tab;

use config_tab::ConfigTab;
use egui_dock::{NodeIndex, SurfaceIndex};
use home_tab::HomeTab;
use tokio::sync::mpsc::UnboundedSender;

use crate::message::Message;

use super::fund::tab::{dashboard::DashboardTab, FundTab};

pub trait Tab {
    fn title(&self) -> WidgetText;
    fn ui(&mut self, ui: &mut Ui);
    fn closeable(&self) -> bool;
}

pub enum TabType {
    Config(ConfigTab),
    Fund(FundTab),
    Home(HomeTab),
    Dashboard(DashboardTab),
}

impl Tab for TabType {
    fn title(&self) -> WidgetText {
        match self {
            TabType::Fund(tab) => tab.title(),
            TabType::Home(tab) => tab.title(),
            TabType::Dashboard(tab) => tab.title(),
            TabType::Config(tab) => tab.title(),
            // Adicione outros tipos de tabs aqui
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.push_id(format!("{}_", self.title().text()), |ui| {
            match self {
                TabType::Fund(tab) => tab.ui(ui),
                TabType::Home(tab) => tab.ui(ui),
                TabType::Dashboard(tab) => tab.ui(ui),
                TabType::Config(tab) => tab.ui(ui),
                // Adicione outros tipos de tabs aqui
            }
        });
    }

    fn closeable(&self) -> bool {
        match self {
            TabType::Fund(tab) => tab.closeable(),
            TabType::Home(tab) => tab.closeable(),
            TabType::Dashboard(tab) => tab.closeable(),
            TabType::Config(tab) => tab.closeable(),
            // Adicione outros tipos de tabs aqui
        }
    }
}

pub struct TabViewer {
    pub open_window: bool,
    pub sender: UnboundedSender<Message>,
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
        let _ = self.sender.send(Message::OpenSearchWindow(true));
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        false
    }
}
