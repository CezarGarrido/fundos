use crate::ui::{charts::stats, tabs::Tab};
use egui::{Frame, Ui, WidgetText};
use egui_extras::{Size, StripBuilder};
use polars::frame::DataFrame;

pub struct DashboardTab {
    pub title: String,
    pub by_year: DataFrame,
    pub by_situation: DataFrame,
    pub by_class: DataFrame,
}

impl DashboardTab {
    pub fn set_dataframes(
        &mut self,
        by_year: DataFrame,
        by_situation: DataFrame,
        by_class: DataFrame,
    ) {
        self.by_year = by_year;
        self.by_situation = by_situation;
        self.by_class = by_class;
    }
}

impl Tab for DashboardTab {
    fn title(&self) -> WidgetText {
        self.title.clone().into()
    }

    fn closeable(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        Frame::none().inner_margin(10.0).show(ui, |ui| {
            StripBuilder::new(ui)
                .size(Size::relative(0.4)) // metade do espaço disponível
                .size(Size::relative(0.6)) // outra metade do espaço disponível
                .vertical(|mut strip| {
                    strip.strip(|builder| {
                        builder
                            .sizes(Size::remainder().at_least(50.0), 2)
                            .horizontal(|mut strip| {
                                strip.cell(|ui| {
                                    ui.group(|ui| {
                                        ui.heading("Quantidade x Ano");
                                        ui.separator();
                                        stats::by_year_bar(&self.by_year, ui);
                                    });
                                });

                                strip.cell(|ui| {
                                    ui.group(|ui| {
                                        ui.heading("Quantidade x Situação");
                                        ui.separator();
                                        stats::by_category_bar(
                                            &self.by_situation,
                                            "SIT",
                                            "TP_FUNDO",
                                            "Situação",
                                            ui,
                                        );
                                    });
                                });
                            });
                    });
                    strip.cell(|ui| {
                        ui.add_space(5.0);
                        ui.group(|ui| {
                            ui.heading("Quantidade x Classe");
                            ui.separator();
                            stats::by_category_bar(
                                &self.by_class,
                                "CLASSE",
                                "TP_FUNDO",
                                "Classe",
                                ui,
                            );
                        });
                    });
                });
        });
    }
}
