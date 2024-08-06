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
                .color(Color32::LIGHT_BLUE)
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

            let colors = generate_colors(category_counter);

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
                        .color(colors[*category_index])
                        .width(0.6)
                        .name(category_name)
                })
                .collect();

            (bar_charts, histogram_data, colors)
        }
        _ => (Vec::new(), Vec::new(), Vec::new()),
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
// Função para gerar cores agradáveis e únicas usando várias paletas de cores
fn generate_colors(n: usize) -> Vec<Color32> {
    // Define as paletas de cores
    let palettes: HashMap<&str, Vec<Color32>> = {
        let mut map = HashMap::new();

        // Paleta Tableau
        map.insert(
            "tableau",
            vec![
                Color32::from_rgb(31, 119, 180),  // azul
                Color32::from_rgb(255, 127, 14),  // laranja
                Color32::from_rgb(44, 160, 44),   // verde
                Color32::from_rgb(214, 39, 40),   // vermelho
                Color32::from_rgb(148, 103, 189), // roxo
                Color32::from_rgb(227, 119, 194), // rosa
                Color32::from_rgb(127, 127, 127), // cinza
                Color32::from_rgb(188, 189, 34),  // amarelo
                Color32::from_rgb(23, 190, 207),  // ciano
            ],
        );

        // Paleta ColorBrewer "Set1"
        map.insert(
            "colorbrewer",
            vec![
                Color32::from_rgb(228, 26, 28),   // vermelho
                Color32::from_rgb(255, 127, 0),   // laranja
                Color32::from_rgb(255, 255, 51),  // amarelo
                Color32::from_rgb(77, 175, 74),   // verde
                Color32::from_rgb(55, 126, 184),  // azul
                Color32::from_rgb(152, 78, 163),  // roxo
                Color32::from_rgb(255, 255, 255), // branco (opcional)
            ],
        );

        // Paleta Viridis
        map.insert(
            "viridis",
            vec![
                Color32::from_rgb(27, 13, 152),   // roxo escuro
                Color32::from_rgb(43, 40, 147),   // azul escuro
                Color32::from_rgb(58, 119, 188),  // azul
                Color32::from_rgb(107, 159, 212), // azul claro
                Color32::from_rgb(162, 206, 225), // azul muito claro
                Color32::from_rgb(218, 238, 250), // azul muito claro
            ],
        );

        // Paleta Cividis
        map.insert(
            "cividis",
            vec![
                Color32::from_rgb(0, 26, 75),     // azul escuro
                Color32::from_rgb(27, 82, 132),   // azul
                Color32::from_rgb(67, 121, 172),  // azul claro
                Color32::from_rgb(120, 159, 202), // azul muito claro
                Color32::from_rgb(189, 212, 237), // azul muito claro
                Color32::from_rgb(242, 242, 242), // cinza muito claro
            ],
        );

        // Paleta Pastel
        map.insert(
            "pastel",
            vec![
                Color32::from_rgb(255, 182, 193), // rosa claro
                Color32::from_rgb(135, 206, 250), // azul claro
                Color32::from_rgb(144, 238, 144), // verde claro
                Color32::from_rgb(255, 255, 224), // amarelo claro
                Color32::from_rgb(255, 160, 122), // laranja claro
                Color32::from_rgb(216, 191, 216), // roxo claro
            ],
        );

        // Paleta D3
        map.insert(
            "d3",
            vec![
                Color32::from_rgb(31, 119, 180),  // azul
                Color32::from_rgb(255, 127, 14),  // laranja
                Color32::from_rgb(44, 160, 44),   // verde
                Color32::from_rgb(214, 39, 40),   // vermelho
                Color32::from_rgb(148, 103, 189), // roxo
                Color32::from_rgb(227, 119, 194), // rosa
            ],
        );

        map
    };

    // Cria um vetor com as chaves das paletas
    let palette_names = [
        "pastel",
        "cividis",
        "tableau",
        "d3",
        "colorbrewer",
        "viridis",
    ];

    let mut colors = Vec::with_capacity(n);

    let mut palette_index = 0;
    let palette_count = palette_names.len();

    while colors.len() < n {
        let palette_name = palette_names[palette_index];
        let palette = palettes.get(palette_name).unwrap();
        let palette_len = palette.len();

        for color in palette.iter().take(palette_len) {
            if colors.len() >= n {
                break;
            }

            colors.push(*color);
        }

        // Muda para a próxima paleta se necessário
        palette_index = (palette_index + 1) % palette_count;
    }

    colors
}
