use std::{collections::HashMap, ops::RangeInclusive};

use egui::{pos2, vec2, Color32, CornerRadius, Sense, Stroke, Ui};
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

pub fn by_category_bar(
    dataframe: &DataFrame,
    category_col: &str,
    value_col: &str,
    x_label: &str,
    ui: &mut Ui,
    height: f32,
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
                    BarChart::new(
                        category_name.clone(),
                        vec![Bar::new(*category_index as f64, *value).width(0.6)],
                    )
                    .color(PIE_COLORS[*category_index % PIE_COLORS.len()])
                    .width(0.6)
                })
                .collect();

            (bar_charts, histogram_data)
        }
        _ => (Vec::new(), Vec::new()),
    };

    let format_data = data.1.clone();
    let x_formatter = move |mark: GridMark, _range: &RangeInclusive<f64>| {
        let v = mark.value as usize;
        let binding = format_data.clone();
        let c = binding.get(v);
        match c {
            Some(d) => d.1.to_string(),
            None => String::new(),
        }
    };

    let y_formatter = |mark: GridMark, _range: &RangeInclusive<f64>| format!("{}", mark.value);

    let x_axes = vec![AxisHints::new_x().label(x_label).formatter(x_formatter)];

    let y_axes = vec![AxisHints::new_y()
        .label("Quantidade")
        .formatter(y_formatter)];

    let legend = Legend::default()
        .position(egui_plot::Corner::LeftTop)
        .text_style(egui::TextStyle::Small);

    Plot::new(format!("plot::funds::{}", x_label.to_lowercase()))
        .legend(legend)
        .show_background(false)
        .show_grid(false)
        .custom_x_axes(x_axes)
        .custom_y_axes(y_axes)
        .height(height)
        .show(ui, |plot_ui| {
            for bar_chart in data.0 {
                plot_ui.bar_chart(bar_chart);
            }
        });
}

/// Paleta de cores para fatias do gráfico de pizza
const PIE_COLORS: [Color32; 10] = [
    Color32::from_rgb(79, 159, 255),  // azul
    Color32::from_rgb(52, 199, 89),   // verde
    Color32::from_rgb(255, 159, 64),  // laranja
    Color32::from_rgb(167, 105, 220), // roxo
    Color32::from_rgb(255, 89, 94),   // vermelho
    Color32::from_rgb(0, 188, 212),   // ciano
    Color32::from_rgb(255, 202, 40),  // âmbar
    Color32::from_rgb(156, 39, 176),  // rosa
    Color32::from_rgb(121, 134, 203), // índigo
    Color32::from_rgb(139, 195, 74),  // verde-claro
];

/// Desenha um gráfico de pizza a partir de um DataFrame com colunas de categoria e valor.
pub fn by_category_pie(
    dataframe: &DataFrame,
    category_col: &str,
    value_col: &str,
    ui: &mut Ui,
    size: f32,
) {
    // ── Extrair dados do DataFrame ──────────────────────────────────────
    let slices: Vec<(String, f32, Color32)> =
        match (dataframe.column(category_col), dataframe.column(value_col)) {
            (Ok(categories), Ok(values)) => {
                let categories = categories
                    .utf8()
                    .expect("Failed to convert category column to utf8");
                let values = values.u32().expect("Failed to convert value column to u32");

                categories
                    .into_no_null_iter()
                    .zip(values.into_no_null_iter())
                    .enumerate()
                    .map(|(i, (cat, val))| {
                        (
                            cat.to_string(),
                            val as f32,
                            PIE_COLORS[i % PIE_COLORS.len()],
                        )
                    })
                    .collect()
            }
            _ => vec![],
        };

    if slices.is_empty() {
        ui.label("Sem dados");
        return;
    }

    let total: f32 = slices.iter().map(|(_, v, _)| v).sum();
    if total == 0.0 {
        ui.label("Sem dados");
        return;
    }

    // ── Layout: pizza + legenda ─────────────────────────────────────────
    let legend_width = 130.0;
    let desired = vec2(size + legend_width, size);
    let (response, painter) = ui.allocate_painter(desired, Sense::hover());

    let center = pos2(response.rect.left() + size / 2.0, response.rect.center().y);
    let radius = (size / 2.0) - 6.0;

    // ── Desenhar fatias ─────────────────────────────────────────────────
    let mut start_angle: f32 = -std::f32::consts::FRAC_PI_2; // começa do topo

    for (_label, value, color) in &slices {
        let sweep = (*value / total) * 2.0 * std::f32::consts::PI;
        let end_angle = start_angle + sweep;

        let steps = (sweep.abs() * 40.0).ceil() as usize + 1;
        let mut points = vec![center];
        for j in 0..=steps {
            let a = start_angle + sweep * (j as f32 / steps as f32);
            points.push(center + vec2(a.cos() * radius, a.sin() * radius));
        }

        painter.add(egui::Shape::convex_polygon(
            points,
            *color,
            Stroke::new(1.0, Color32::from_gray(60)),
        ));

        start_angle = end_angle;
    }

    // furo central (donut)
    let inner_r = radius * 0.42;
    painter.circle_filled(center, inner_r, Color32::from_gray(32));

    // texto central: total
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        format!("{:.0}", total),
        egui::FontId::proportional(18.0),
        if ui.visuals().dark_mode {
            Color32::WHITE
        } else {
            Color32::from_gray(20)
        },
    );

    // ── Legenda ─────────────────────────────────────────────────────────
    let legend_x = response.rect.left() + size + 12.0;
    let mut y = response.rect.top() + 8.0;
    let item_h = 18.0;

    for (label, value, color) in &slices {
        let pct = (*value / total) * 100.0;
        let rect = egui::Rect::from_min_size(pos2(legend_x, y), vec2(10.0, 10.0));
        painter.rect_filled(rect, CornerRadius::same(2), *color);

        let label_text = format!("{} ({:.1}%)", label, pct);
        painter.text(
            pos2(legend_x + 16.0, y + 5.0),
            egui::Align2::LEFT_TOP,
            label_text,
            egui::FontId::proportional(11.0),
            if ui.visuals().dark_mode {
                Color32::from_gray(210)
            } else {
                Color32::from_gray(60)
            },
        );

        y += item_h;
        if y + item_h > response.rect.bottom() {
            break;
        }
    }
}
