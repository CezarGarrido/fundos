use egui::{Frame, Ui, WidgetText};
pub mod home_tab;

use home_tab::HomeTab;
use tokio::sync::mpsc::UnboundedSender;

use crate::message::Message;

use super::fund::tab::{
    ativos::AssetsMarketTab, dashboard::DashboardTab, historico::HistoricoTab, FundTab,
};

pub trait Tab {
    fn title(&self) -> WidgetText;
    fn ui(&mut self, ui: &mut Ui);
    fn closeable(&self) -> bool;
}

pub enum TabType {
    Fund(FundTab),
    Home(HomeTab),
    Dashboard(DashboardTab),
    Ativos(AssetsMarketTab),
    Historico(HistoricoTab),
}

impl Tab for TabType {
    fn title(&self) -> WidgetText {
        match self {
            TabType::Fund(tab) => tab.title(),
            TabType::Home(tab) => tab.title(),
            TabType::Dashboard(tab) => tab.title(),
            TabType::Ativos(tab) => tab.title(),
            TabType::Historico(tab) => tab.title(),
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.push_id(format!("{}_", self.title().text()), |ui| {
            Frame::NONE
                .fill(ui.style().visuals.panel_fill)
                .inner_margin(-2.0)
                .outer_margin(0.0)
                .show(ui, |ui| match self {
                    TabType::Fund(tab) => tab.ui(ui),
                    TabType::Home(tab) => tab.ui(ui),
                    TabType::Dashboard(tab) => tab.ui(ui),
                    TabType::Ativos(tab) => tab.ui(ui),
                    TabType::Historico(tab) => tab.ui(ui),
                });
        });
    }

    fn closeable(&self) -> bool {
        match self {
            TabType::Fund(tab) => tab.closeable(),
            TabType::Home(tab) => tab.closeable(),
            TabType::Dashboard(tab) => tab.closeable(),
            TabType::Ativos(tab) => tab.closeable(),
            TabType::Historico(tab) => tab.closeable(),
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

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        tab.closeable()
    }

    fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
        tab.closeable()
    }

    fn on_add(&mut self, _path: egui_dock::NodePath) {
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
