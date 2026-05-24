use crate::ui::design::Scale;
use std::ops::RangeInclusive;

use chrono::DateTime;
use egui::{Color32, Ui};
use egui_plot::{AxisHints, GridMark, Legend, Line, Plot};
use fundos_common::types::TimeSeriesPoint;

pub struct Indice {
    pub name: String,
    pub color: Color32,
    pub series: Vec<TimeSeriesPoint>,
}

pub fn chart(series: &[TimeSeriesPoint], indices: Vec<Indice>, ui: &mut Ui) {
    let line_color = ui.visuals().text_color();
    let mut last_point: Option<[f64; 2]> = None;

    let mut line_data: Vec<[f64; 2]> = Vec::with_capacity(series.len());
    for point in series {
        let timestamp = point
            .date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        line_data.push([timestamp, point.value]);
        last_point = Some([timestamp, point.value]);
    }

    let chart = Line::new("Fundo", line_data)
        .color(line_color)
        .width(1.5)
        .fill(0.0);

    let mut charts_data = Vec::new();
    for indice in indices.iter() {
        let mut last_idx_point = None;
        let mut idx_line_data: Vec<[f64; 2]> = Vec::with_capacity(indice.series.len());
        for point in &indice.series {
            let timestamp = point
                .date
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp() as f64;

            // Only include index points up to the fund's last date
            if let Some(last) = last_point {
                if timestamp > last[0] {
                    continue;
                }
            }

            idx_line_data.push([timestamp, point.value]);
            last_idx_point = Some([timestamp, point.value]);
        }

        let line = Line::new(indice.name.clone(), idx_line_data)
            .color(indice.color)
            .width(1.2);

        charts_data.push((line, last_idx_point, indice.color));
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
                            .size(Scale::DEFAULT.label())
                            .strong(),
                    )
                    .anchor(egui::Align2::RIGHT_CENTER);
                    plot_ui.text(text);
                }
            }

            if let Some(last) = last_point {
                plot_ui.hline(
                    egui_plot::HLine::new("", last[1])
                        .color(line_color)
                        .style(egui_plot::LineStyle::Dotted { spacing: 4.0 }),
                );
                let text = egui_plot::Text::new(
                    "",
                    egui_plot::PlotPoint::new(last[0], last[1]),
                    egui::RichText::new(format!(" {:.2}% ", last[1]))
                        .background_color(strong_text_color)
                        .color(window_fill)
                        .size(Scale::DEFAULT.button())
                        .strong(),
                )
                .anchor(egui::Align2::RIGHT_CENTER);
                plot_ui.text(text);
            }
        });
}
