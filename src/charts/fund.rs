use std::ops::RangeInclusive;

use chrono::DateTime;
use egui::Ui;
use egui_plot::{AxisHints, GridMark, Legend, Line, Plot};
use polars::frame::DataFrame;

//TODO: refatorar para unificar as datas
pub fn profit_chart(dataframe: &DataFrame, cdi: &DataFrame, ui: &mut Ui) {
    let chart = match (dataframe.column("DT_COMPTC"), dataframe.column("RENT_ACUM")) {
        (Ok(dates), Ok(rentabilidade)) => {
            let mut line_data = Vec::new();
            let dates = dates.utf8().unwrap();
            let rentabilidade = rentabilidade.f64().unwrap();
            for (date, rent) in dates.into_iter().zip(rentabilidade.into_iter()) {
                if let (Some(date), Some(rent)) = (date, rent) {
                    if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
                        let timestamp = parsed_date
                            .and_hms_opt(0, 0, 0)
                            .unwrap()
                            .and_utc()
                            .timestamp() as f64;
                        line_data.push([timestamp, rent]);
                    }
                }
            }

            Line::new(line_data)
                .color(egui::Color32::GREEN)
                .name("Fundo")
        }
        _ => Line::new(Vec::new())
            .color(egui::Color32::GREEN)
            .name("Fundo"),
    };

    let chart_cdi = match (cdi.column("data"), cdi.column("cdi_acumulado")) {
        (Ok(dates), Ok(rentabilidade)) => {
            let mut line_data = Vec::new();
            let dates = dates.utf8().unwrap();

            let rentabilidade = rentabilidade.f64().unwrap();

            for (date, rent) in dates.into_iter().zip(rentabilidade.into_iter()) {
                if let (Some(date), Some(rent)) = (date, rent) {
                    if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(date, "%d/%m/%Y") {
                        let timestamp = parsed_date
                            .and_hms_opt(0, 0, 0)
                            .unwrap()
                            .and_utc()
                            .timestamp() as f64;
                        line_data.push([timestamp, rent]);
                    }
                }
            }

            Line::new(line_data).color(egui::Color32::RED).name("CDI")
        }
        _ => Line::new(Vec::new()).color(egui::Color32::RED).name("CDI"),
    };
    let x_formatter = |mark: GridMark, _digits, _range: &RangeInclusive<f64>| {
        let timestamp = mark.value as i64;
        if timestamp <= 0 {
            "".to_owned()
        } else if let Some(datetime) = DateTime::from_timestamp(timestamp, 0) {
            format!("{}", datetime.format("%d/%m/%Y")) // Assume timezone offset of 0 for simplicity
        } else {
            "".to_owned()
        }
    };

    let y_formatter =
        |mark: GridMark, _digits, _range: &RangeInclusive<f64>| format!("{}%", mark.value);

    let x_axes = vec![AxisHints::new_x().label("").formatter(x_formatter)];

    let y_axes = vec![AxisHints::new_y().label("").formatter(y_formatter)];

    Plot::new("plot::funds::profit")
        .legend(Legend::default())
        .set_margin_fraction(egui::Vec2::new(0.0, 0.15))
        .y_axis_position(egui_plot::HPlacement::Left)
        .y_axis_width(0)
        .custom_x_axes(x_axes)
        .custom_y_axes(y_axes)
        .include_y(0.0)
        .label_formatter(|name, value| {
            if !name.is_empty() {
                if let Some(datetime) = DateTime::from_timestamp(value.x as i64, 0) {
                    let dt = format!("{}", datetime.format("%d/%m/%Y")); // Assume timezone offset of 0 for simplicity
                    format!("{}: ({}, {:.*}%)", name, dt, 2, value.y)
                } else {
                    "".to_owned()
                }
            } else {
                "".to_owned()
            }
        })
        .height(400.0)
        .show(ui, |plot_ui| {
            plot_ui.line(chart);
            plot_ui.line(chart_cdi);
        });
}
