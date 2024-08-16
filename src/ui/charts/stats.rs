use std::{collections::HashMap, ops::RangeInclusive};

use egui::{Color32, Ui};
use egui_plot::{AxisHints, Bar, BarChart, GridMark, Legend, Plot};
use polars::frame::DataFrame;

pub fn by_year_bar(dataframe: &DataFrame, ui: &mut Ui) {
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

            BarChart::new(histogram_data)
                //.color(Color32::LIGHT_BLUE)
                .name("Fundos por Ano")
        }
        _ => BarChart::new(Vec::new())
            .color(Color32::LIGHT_BLUE)
            .name("Fundos por Ano"),
    };

    let x_formatter = |mark: GridMark, _digits, _range: &RangeInclusive<f64>| {
        let year = mark.value as i32;
        if year < 0 {
            String::new() // No labels for negative years
        } else {
            format!("{}", year)
        }
    };

    let y_formatter =
        |mark: GridMark, _digits, _range: &RangeInclusive<f64>| format!("{}", mark.value);

    let x_axes = vec![AxisHints::new_x().label("Ano").formatter(x_formatter)];

    let y_axes = vec![AxisHints::new_y()
        .label("Quantidade")
        .formatter(y_formatter)];

    Plot::new("plot::funds:year")
        .legend(Legend::default())
        .custom_x_axes(x_axes)
        .custom_y_axes(y_axes)
        .show(ui, |plot_ui| plot_ui.bar_chart(chart));
}

pub fn by_category_bar(
    dataframe: &DataFrame,
    category_col: &str,
    value_col: &str,
    x_label: &str,
    ui: &mut Ui,
) {
    let data = match (dataframe.column(category_col), dataframe.column(value_col)) {
        (Ok(categories), Ok(values)) => {
            let categories = categories
                .utf8()
                .expect("Failed to convert category column to utf8");
            let values = values.u32().expect("Failed to convert value column to u32");

            // Mapear categorias para valores numéricos
            let mut category_map: HashMap<String, usize> = HashMap::new();
            let mut category_counter = 0;
            for category in categories.into_no_null_iter() {
                if !category_map.contains_key(category) {
                    category_map.insert(category.to_string(), category_counter);
                    category_counter += 1;
                }
            }

            let histogram_data: Vec<(usize, String, f64)> = categories
                .into_no_null_iter()
                .zip(values.into_no_null_iter())
                .map(|(category, value)| {
                    let category_index = *category_map.get(category).unwrap();
                    (category_index, category.to_string(), value as f64)
                })
                .collect();

            let bar_charts: Vec<BarChart> = histogram_data
                .iter()
                .map(|(category_index, category_name, value)| {
                    BarChart::new(vec![Bar::new(*category_index as f64, *value).width(0.6)])
                        //.color(colors[*category_index])
                        .width(0.6)
                        .name(category_name)
                })
                .collect();

            (bar_charts, histogram_data)
        }
        _ => (Vec::new(), Vec::new()),
    };

    let format_data = data.1.clone();
    let x_formatter = move |mark: GridMark, _digits, _range: &RangeInclusive<f64>| {
        let v = mark.value as usize;
        let binding = format_data.clone();
        let c = binding.get(v);
        match c {
            Some(d) => d.1.to_string(),
            None => String::new(),
        }
    };

    let y_formatter =
        |mark: GridMark, _digits, _range: &RangeInclusive<f64>| format!("{}", mark.value);

    let x_axes = vec![AxisHints::new_x().label(x_label).formatter(x_formatter)];

    let y_axes = vec![AxisHints::new_y()
        .label("Quantidade")
        .formatter(y_formatter)];

    let legend = Legend::default()
        .position(egui_plot::Corner::LeftTop)
        .text_style(egui::TextStyle::Small);

    Plot::new(format!("plot::funds::{}", x_label.to_lowercase()))
        .legend(legend)
        .custom_x_axes(x_axes)
        .custom_y_axes(y_axes)
        .show(ui, |plot_ui| {
            for bar_chart in data.0 {
                plot_ui.bar_chart(bar_chart);
            }
        });
}
