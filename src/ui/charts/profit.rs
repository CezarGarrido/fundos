use std::ops::RangeInclusive;

use chrono::DateTime;
use egui::{Color32, Ui};
use egui_plot::{AxisHints, GridMark, Legend, Line, Plot};
use polars::frame::DataFrame;

pub struct Indice {
    pub name: String,
    pub color: Color32,
    pub dataframe: DataFrame,
}

///TODO: refatorar para unificar as datas
pub fn chart(dataframe: &DataFrame, indices: Vec<Indice>, ui: &mut Ui) {
    let color = ui.visuals().selection.bg_fill;

    let line_color = ui.visuals().text_color();
    let mut last_point: Option<[f64; 2]> = None;

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
                        last_point = Some([timestamp, rent]);
                    }
                }
            }

            Line::new("Fundo", line_data)
                .color(line_color)
                .width(1.5)
                .fill(0.0)
        }
        _ => Line::new("Fundo", Vec::new()).color(color).width(1.5),
    };

    let mut charts_data = Vec::new();
    for indice in indices.iter() {
        let mut last_idx_point = None;
        let chart = match (
            indice.dataframe.column("date"),
            indice.dataframe.column("value"),
        ) {
            (Ok(dates), Ok(rentabilidade)) => {
                let mut line_data = Vec::new();
                let dates = dates.utf8().unwrap();
                let rentabilidade = rentabilidade.f64().unwrap();
                for (date, rent) in dates.into_iter().zip(rentabilidade.into_iter()) {
                    if let (Some(date), Some(rent)) = (date, rent) {
                        if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(date, "%d/%m/%Y")
                        {
                            let timestamp = parsed_date
                                .and_hms_opt(0, 0, 0)
                                .unwrap()
                                .and_utc()
                                .timestamp() as f64;

                            if let Some(last) = last_point {
                                if timestamp > last[0] {
                                    continue;
                                }
                            }

                            line_data.push([timestamp, rent]);
                            last_idx_point = Some([timestamp, rent]);
                        }
                    }
                }
                Line::new(indice.name.to_string(), line_data)
                    .color(indice.color)
                    .width(1.2)
            }
            _ => Line::new("", Vec::new()).width(1.2),
        };

        charts_data.push((chart, last_idx_point, indice.color));
    }

    let x_formatter = |mark: GridMark, _range: &RangeInclusive<f64>| {
        let timestamp = mark.value as i64;
        if timestamp <= 0 {
            "".to_owned()
        } else if let Some(datetime) = DateTime::from_timestamp(timestamp, 0) {
            format!("{}", datetime.format("%d/%m/%Y"))
        } else {
            "".to_owned()
        }
    };

    let y_formatter = |mark: GridMark, _range: &RangeInclusive<f64>| format!("{:.2}%", mark.value);

    let x_axes = vec![AxisHints::new_x().label("").formatter(x_formatter)];
    let y_axes = vec![AxisHints::new_y()
        .label("")
        .formatter(y_formatter)
        .placement(egui_plot::Placement::RightTop)];

    let strong_text_color = ui.visuals().strong_text_color();
    let window_fill = ui.visuals().window_fill();

    Plot::new("plot::funds::profit")
        .legend(Legend::default())
        .show_background(false)
        .set_margin_fraction(egui::Vec2::new(0.0, 0.15))
        .y_axis_position(egui_plot::HPlacement::Right)
        .y_axis_min_width(0.0)
        .custom_x_axes(x_axes)
        .custom_y_axes(y_axes)
        .include_y(0.0)
        .label_formatter(|name, value| {
            if !name.is_empty() {
                if let Some(datetime) = DateTime::from_timestamp(value.x as i64, 0) {
                    let dt = format!("{}", datetime.format("%d/%m/%Y"));
                    format!("{}: ({}, {:.*}%)", name, dt, 2, value.y)
                } else {
                    "".to_owned()
                }
            } else {
                "".to_owned()
            }
        })
        .show(ui, |plot_ui| {
            plot_ui.line(chart);
            for (chart, last_idx_point, color) in charts_data {
                plot_ui.line(chart);
                if let Some(last) = last_idx_point {
                    plot_ui.hline(
                        egui_plot::HLine::new("", last[1])
                            .color(color)
                            .style(egui_plot::LineStyle::Dotted { spacing: 4.0 }),
                    );
                    let text = egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(last[0], last[1]),
                        egui::RichText::new(format!(" {:.2}% ", last[1]))
                            .background_color(color)
                            .color(window_fill)
                            .size(11.0)
                            .strong(),
                    )
                    .anchor(egui::Align2::RIGHT_CENTER);
                    plot_ui.text(text);
                }
            }

            if let Some(last) = last_point {
                // Add a dotted horizontal line at the last value
                plot_ui.hline(
                    egui_plot::HLine::new("", last[1])
                        .color(line_color)
                        .style(egui_plot::LineStyle::Dotted { spacing: 4.0 }),
                );
                // Draw a text label slightly to the right of the last point
                let text = egui_plot::Text::new(
                    "",
                    egui_plot::PlotPoint::new(last[0], last[1]),
                    egui::RichText::new(format!(" {:.2}% ", last[1]))
                        .background_color(strong_text_color)
                        .color(window_fill)
                        .size(13.0)
                        .strong(),
                )
                .anchor(egui::Align2::RIGHT_CENTER);
                plot_ui.text(text);
            }
        });
}
