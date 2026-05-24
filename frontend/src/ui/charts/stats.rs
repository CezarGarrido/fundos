use std::ops::RangeInclusive;

use egui::{Color32, Ui};
use egui_plot::{AxisHints, Bar, BarChart, GridMark, Legend, Plot};
use fundos_common::types::YearStat;

pub fn by_year_bar(stats: &[YearStat], ui: &mut Ui, height: f32) {
    let histogram_data: Vec<Bar> = stats
        .iter()
        .filter_map(|ys| {
            ys.year.parse::<i32>().ok().map(|year| {
                Bar::new(year as f64, ys.count as f64).width(0.6)
            })
        })
        .filter(|bar| bar.argument > 0.0)
        .collect();

    let chart =
        BarChart::new("Fundos por Ano", histogram_data).color(Color32::from_rgb(37, 99, 235));

    let x_formatter = |mark: GridMark, _range: &RangeInclusive<f64>| {
        let year = mark.value as i32;
        if year < 0 {
            String::new()
        } else {
            format!("{}", year)
        }
    };
    let y_formatter = |mark: GridMark, _range: &RangeInclusive<f64>| format!("{}", mark.value);
    let x_axes = vec![AxisHints::new_x().label("Ano").formatter(x_formatter)];
    let y_axes = vec![AxisHints::new_y()
        .label("Quantidade")
        .formatter(y_formatter)];

    Plot::new("plot::funds:year")
        .legend(Legend::default())
        .show_background(false)
        .show_grid(false)
        .y_axis_min_width(0.0)
        .custom_x_axes(x_axes)
        .custom_y_axes(y_axes)
        .height(height)
        .include_y(0.0)
        .show(ui, |plot_ui| plot_ui.bar_chart(chart));
}
