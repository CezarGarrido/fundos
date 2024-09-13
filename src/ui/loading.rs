pub fn show(ui: &mut egui::Ui) {
    // let errored = assets.load_progress.errored();
   // egui::CentralPanel::default()
       // .frame(egui::Frame::default())
       // .show_inside(ui, |ui| {
            let height = 40.0; //ui.available_height();
            let ctx = ui.ctx().clone();

            let space_size = 0.03;
            let spinner_size = 0.10;
            let text_size = 0.034;
            ui.vertical_centered(|ui| {
                ui.add_space(height * 0.3);

                let rect = ui
                    .label(
                        egui::RichText::new(egui_phosphor::regular::CHART_BAR)
                            //.color(egui::Color32::WHITE)
                            .size(height),
                    )
                    .rect;
                egui::Spinner::new().paint_at(ui, rect.expand(spinner_size * height * 0.2));
                ui.add_space(height * space_size);
                ui.label(
                    egui::RichText::new("Carregando")
                        .color(egui::Color32::WHITE)
                        .size(height * text_size),
                );
            });

            ctx.data_mut(|d| {
                d.insert_temp(ui.id(), (spinner_size, space_size, text_size));
            })
    //    });
}
