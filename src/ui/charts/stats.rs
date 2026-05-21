use std::ops::RangeInclusive;

use egui::{Color32, Ui};
use egui_plot::{AxisHints, Bar, BarChart, GridMark, Legend, Plot};
use polars::frame::DataFrame;

pub fn by_year_bar(dataframe: &DataFrame, ui: &mut Ui, height: f32) {
    let chart = match (dataframe.column("Ano"), dataframe.column("Quant")) {
        (Ok(years), Ok(counts)) => {
            let years = years.i32().expect("Failed to convert 'Ano' column to i32");
            let counts = counts
                .u32()
                .expect("Failed to convert 'Quant' column to u32");

            let histogram_data: Vec<Bar> = years
                .into_no_null_iter()
                .zip(counts.into_no_null_iter())
                .filter(|(year, _)| *year > 0)
                .map(|(year, count)| Bar::new(year as f64, count as f64).width(0.6))
                .collect();

            BarChart::new("Fundos por Ano", histogram_data).color(Color32::from_rgb(37, 99, 235))
        }
        _ => BarChart::new("Fundos por Ano", Vec::new()).color(Color32::from_rgb(37, 99, 235)),
    };

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
