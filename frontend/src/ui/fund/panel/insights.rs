use crate::ui::design::Scale;
use crate::util;
use egui::Ui;
use fundos_common::types::{PortfolioAsset, PortfolioPL};

pub fn show_kpis(assets: &[PortfolioAsset], pl: &[PortfolioPL], ui: &mut Ui) {
    let (heading_color, _text_color, secondary_color, success_color, card_bg) =
        if ui.visuals().dark_mode {
            (
                egui::Color32::from_rgb(255, 255, 255),
                egui::Color32::from_rgb(230, 235, 245),
                egui::Color32::from_rgb(175, 185, 200),
                egui::Color32::from_rgb(46, 204, 113),
                egui::Color32::from_rgb(30, 35, 45),
            )
        } else {
            (
                egui::Color32::from_rgb(10, 15, 25),
                egui::Color32::from_rgb(30, 35, 45),
                egui::Color32::from_rgb(80, 85, 100),
                egui::Color32::from_rgb(19, 115, 51),
                egui::Color32::from_rgb(245, 247, 250),
            )
        };

    if assets.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::LIGHTBULB.to_string())
                    .color(egui::Color32::from_rgb(250, 185, 80))
                    .size(Scale::ICON_JUMBO),
            );
            ui.add_space(15.0);
            ui.heading(
                egui::RichText::new("Aguardando Dados da Carteira")
                    .color(heading_color)
                    .size(Scale::DEFAULT.metric()),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Por favor, selecione e carregue uma data de referência na aba 'Carteira' primeiro para gerar os insights automáticos da carteira.")
                    .color(secondary_color)
                    .size(Scale::DEFAULT.button()),
            );
        });
        return;
    }

    let max_pos = get_largest_position(assets);
    let total_assets_count = get_unique_assets_count(assets);

    ui.columns(3, |cols| {
        draw_kpi_card(
            &mut cols[0],
            "GRAU DE DIVERSIFICAÇÃO",
            &format!("{} Ativos", total_assets_count),
            if total_assets_count > 30 {
                "Alta diversificação (Risco diluído)"
            } else if total_assets_count > 10 {
                "Diversificação moderada (Equilibrada)"
            } else {
                "Carteira concentrada (Foco em convicção)"
            },
            success_color,
            card_bg,
        );

        let max_pct = max_pos.as_ref().map(|p| p.pct).unwrap_or(0.0);
        draw_kpi_card(
            &mut cols[1],
            "MAIOR CONCENTRAÇÃO",
            &format!("{:.2}% do PL", max_pct),
            if max_pct > 20.0 {
                "Concentração alta (>20% do PL)"
            } else if max_pct > 10.0 {
                "Exposição relevante (Teto privado 10%)"
            } else {
                "Dispersão excelente (<10% por ativo)"
            },
            if max_pct > 20.0 {
                egui::Color32::from_rgb(231, 76, 60)
            } else {
                egui::Color32::from_rgb(52, 152, 219)
            },
            card_bg,
        );

        let pl_val = pl.first().map(|p| p.patrimonio_liquido).unwrap_or(0.0);
        let pl_formatted = if pl_val > 0.0 {
            util::to_real(pl_val)
        } else {
            "R$ N/A".to_string()
        };
        draw_kpi_card(
            &mut cols[2],
            "PATRIMÔNIO LÍQUIDO",
            &pl_formatted,
            "Total sob gestão deste fundo",
            egui::Color32::from_rgb(155, 89, 182),
            card_bg,
        );
    });
}

pub fn show_detailed(
    assets: &[PortfolioAsset],
    pl: &[PortfolioPL],
    ui: &mut Ui,
) {
    if assets.is_empty() {
        return;
    }

    let (heading_color, text_color, secondary_color, success_color, _card_bg) =
        if ui.visuals().dark_mode {
            (
                egui::Color32::from_rgb(255, 255, 255),
                egui::Color32::from_rgb(230, 235, 245),
                egui::Color32::from_rgb(175, 185, 200),
                egui::Color32::from_rgb(46, 204, 113),
                egui::Color32::from_rgb(30, 35, 45),
            )
        } else {
            (
                egui::Color32::from_rgb(10, 15, 25),
                egui::Color32::from_rgb(30, 35, 45),
                egui::Color32::from_rgb(80, 85, 100),
                egui::Color32::from_rgb(19, 115, 51),
                egui::Color32::from_rgb(245, 247, 250),
            )
        };

    let max_pos = get_largest_position(assets);
    let (short_term_pct, long_term_pct) = get_duration_split(assets);

    ui.vertical(|ui| {
        ui.vertical(|ui| {
            ui.heading(
                egui::RichText::new(format!(
                    "{} Posição de Maior Convicção",
                    egui_phosphor::regular::TARGET
                ))
                .size(Scale::DEFAULT.body())
                .strong()
                .color(heading_color),
            );
            ui.separator();
            ui.add_space(8.0);

            if let Some(pos) = &max_pos {
                ui.label(
                    egui::RichText::new(&pos.name)
                        .size(Scale::DEFAULT.button())
                        .strong()
                        .color(ui.visuals().selection.bg_fill),
                );
                ui.add_space(8.0);

                egui::Grid::new("largest_pos_grid")
                    .striped(true)
                    .num_columns(2)
                    .min_col_width(ui.available_width() / 2.0 - 5.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Exposição Absoluta:").color(text_color));
                        ui.label(
                            egui::RichText::new(util::to_real(pos.value))
                                .strong()
                                .color(text_color),
                        );
                        ui.end_row();

                        ui.label(egui::RichText::new("Exposição no PL:").color(text_color));
                        ui.label(
                            egui::RichText::new(format!("{:.3}%", pos.pct))
                                .strong()
                                .color(success_color),
                        );
                        ui.end_row();

                        ui.label(egui::RichText::new("Tipo de Ativo:").color(text_color));
                        ui.label(
                            egui::RichText::new(&pos.tipo_ativo)
                                .color(secondary_color),
                        );
                        ui.end_row();

                        if pos.cost > 0.0 {
                            ui.label(
                                egui::RichText::new("Custo de Aquisição:").color(text_color),
                            );
                            ui.label(
                                egui::RichText::new(util::to_real(pos.cost))
                                    .color(text_color),
                            );
                            ui.end_row();

                            let profit_pct = ((pos.value / pos.cost) - 1.0) * 100.0;
                            let profit_value = pos.value - pos.cost;
                            ui.label(
                                egui::RichText::new("Resultado Acumulado:").color(text_color),
                            );
                            let color = if profit_value >= 0.0 {
                                if ui.visuals().dark_mode {
                                    egui::Color32::from_rgb(46, 204, 113)
                                } else {
                                    egui::Color32::from_rgb(19, 115, 51)
                                }
                            } else {
                                egui::Color32::from_rgb(231, 76, 60)
                            };
                            let icon = if profit_value >= 0.0 { "▲" } else { "▼" };
                            let formatted = util::to_real(profit_value.abs());
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} R$ {} ({:+.2}%)",
                                    icon,
                                    formatted.replace("R$", ""),
                                    profit_pct
                                ))
                                .strong()
                                .color(color),
                            );
                            ui.end_row();
                        }
                    });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(15.0);

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(
                                egui_phosphor::regular::SHIELD_CHECK.to_string(),
                            )
                            .color(success_color),
                        );
                        ui.label(
                            egui::RichText::new("Análise de Risco de Liquidez")
                                .strong()
                                .size(Scale::DEFAULT.small_text())
                                .color(heading_color),
                        );
                    });
                    ui.separator();
                    let msg = if pos.pct > 20.0 {
                        "ATENÇÃO: A alocação neste ativo excede 20% do PL total do fundo."
                    } else if pos.pct > 10.0 {
                        "ALERTA: Posição acima de 10%."
                    } else {
                        "CONCORDÂNCIA: A maior posição está enquadrada abaixo do limite de 10%."
                    };
                    ui.label(
                        egui::RichText::new(msg)
                            .size(Scale::DEFAULT.label())
                            .color(text_color),
                    );
                });

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(15.0);

                // Analytics section — computed from local portfolio data
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(egui_phosphor::regular::BRAIN.to_string())
                                .color(egui::Color32::from_rgb(155, 89, 182)),
                        );
                        ui.label(
                            egui::RichText::new("Comportamento do Gestor (Analytics)")
                                .strong()
                                .size(Scale::DEFAULT.small_text())
                                .color(heading_color),
                        );
                    });
                    ui.separator();
                    ui.add_space(6.0);

                    let top3_pct: f64 = {
                        let mut pcts: Vec<f64> = assets.iter().map(|a| a.vl_porcentagem_pl).collect();
                        pcts.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
                        pcts.iter().take(3).sum()
                    };
                    let pl_total = pl.first().map(|p| p.patrimonio_liquido).unwrap_or(0.0);
                    let n_ativos = get_unique_assets_count(assets);
                    let n_classes = {
                        let mut types: Vec<&str> = assets.iter().map(|a| a.tipo_ativo.as_str()).collect();
                        types.sort_unstable();
                        types.dedup();
                        types.len()
                    };

                    ui.label(egui::RichText::new(format!(
                        "Concentração Top-3: {:.1}% do PL  |  Patrimônio Líquido: {}  |  {} ativos em {} classes",
                        top3_pct,
                        util::to_real(pl_total),
                        n_ativos,
                        n_classes,
                    ))
                    .size(Scale::DEFAULT.label())
                    .color(text_color));

                    ui.add_space(6.0);
                    let concentration_msg = if top3_pct > 60.0 {
                        "PERFIL CONCENTRADO: O gestor aposta forte em poucas posições (top-3 > 60% do PL). Estratégia de convicção com risco de concentração elevado."
                    } else if top3_pct > 35.0 {
                        "PERFIL MODERADO: Concentração média nas principais posições. Equilíbrio entre convicção e diversificação."
                    } else {
                        "PERFIL DIVERSIFICADO: Baixa concentração nas maiores posições. Estratégia pulverizada com menor risco idiossincrático."
                    };
                    ui.label(
                        egui::RichText::new(concentration_msg)
                            .size(Scale::DEFAULT.label())
                            .color(secondary_color),
                    );
                });
            } else {
                ui.label(
                    egui::RichText::new("Sem dados específicos de ativos na carteira.")
                        .color(secondary_color),
                );
            }
        });

        // Bloco Inferior: Análise de Prazo e Liquidez
        ui.add_space(15.0);
        ui.separator();
        ui.add_space(15.0);
        ui.vertical(|ui| {
            ui.heading(
                egui::RichText::new(format!(
                    "{} Alocação de Prazo e Liquidez",
                    egui_phosphor::regular::CLOCK
                ))
                .size(Scale::DEFAULT.body())
                .strong()
                .color(heading_color),
            );
            ui.separator();
            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("Perfil Estrutural de Liquidez:")
                    .strong()
                    .color(heading_color),
            );
            ui.add_space(10.0);

            let total_pct = short_term_pct + long_term_pct;
            let short_ratio = if total_pct > 0.0 {
                short_term_pct / total_pct
            } else {
                0.5
            };

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Liquidez / Curto Prazo")
                        .size(Scale::DEFAULT.label())
                        .color(secondary_color),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("Estratégico / Longo Prazo")
                            .size(Scale::DEFAULT.label())
                            .color(secondary_color),
                    );
                });
            });

            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 16.0),
                egui::Sense::hover(),
            );
            let painter = ui.painter();
            let orange_color = if ui.visuals().dark_mode {
                egui::Color32::from_rgb(230, 126, 34)
            } else {
                egui::Color32::from_rgb(190, 95, 10)
            };
            painter.rect_filled(rect, egui::CornerRadius::same(4), orange_color);

            let short_width = rect.width() * short_ratio as f32;
            let short_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(short_width, rect.height()),
            );
            painter.rect_filled(short_rect, egui::CornerRadius::same(4), success_color);

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("● {:.1}%", short_term_pct))
                        .color(success_color)
                        .strong()
                        .size(Scale::DEFAULT.small_text()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("● {:.1}%", long_term_pct))
                            .color(orange_color)
                            .strong()
                            .size(Scale::DEFAULT.small_text()),
                    );
                });
            });

            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("Interpretação da Duration / Liquidez:")
                    .strong()
                    .size(Scale::DEFAULT.small_text())
                    .color(heading_color),
            );
            ui.separator();
            let profile_msg = if short_term_pct > 70.0 {
                "CARTEIRA ULTRA-LÍQUIDA: Mais de 70% da carteira está alocada em instrumentos de alta liquidez."
            } else if long_term_pct > 70.0 {
                "CARTEIRA DE VALOR / LONGO PRAZO: Mais de 70% da carteira foca em ativos estruturais."
            } else {
                "PERFIL EQUILIBRADO / HÍBRIDO: O fundo equilibra liquidez rápida com investimentos de longo prazo."
            };
            ui.label(
                egui::RichText::new(profile_msg)
                    .size(Scale::DEFAULT.label())
                    .color(text_color),
            );
        });
    });
}

struct LargestPos {
    name: String,
    codigo: String,
    pct: f64,
    value: f64,
    cost: f64,
    tipo_ativo: String,
}

fn get_largest_position(assets: &[PortfolioAsset]) -> Option<LargestPos> {
    if assets.is_empty() {
        return None;
    }

    let max_asset = assets
        .iter()
        .max_by(|a, b| a.vl_porcentagem_pl.partial_cmp(&b.vl_porcentagem_pl).unwrap())
        .unwrap();

    if max_asset.vl_porcentagem_pl <= 0.0 {
        return None;
    }

    // Compute cost from market value minus profit; if not available, use 0
    // The old code had VL_CUSTO_POS_FINAL column which is a different metric
    let cost = 0.0; // Custo not directly available in PortfolioAsset

    Some(LargestPos {
        name: if max_asset.nome_ativo.is_empty() {
            max_asset.codigo_isin.clone()
        } else {
            max_asset.nome_ativo.clone()
        },
        codigo: max_asset.codigo_isin.clone(),
        pct: max_asset.vl_porcentagem_pl,
        value: max_asset.valor_mercado,
        cost,
        tipo_ativo: max_asset.tipo_ativo.clone(),
    })
}

fn get_unique_assets_count(assets: &[PortfolioAsset]) -> usize {
    if assets.is_empty() {
        return 0;
    }
    // Count unique by codigo_isin
    let mut codes: Vec<&str> = assets.iter().map(|a| a.codigo_isin.as_str()).collect();
    codes.sort_unstable();
    codes.dedup();
    codes.len()
}

fn get_duration_split(assets: &[PortfolioAsset]) -> (f64, f64) {
    if assets.is_empty() {
        return (50.0, 50.0);
    }

    let short_keywords = [
        "títulos públicos",
        "operações compromissadas",
        "caixa",
        "disponibilidades",
        "renda fixa",
    ];

    let (short, long): (f64, f64) = assets
        .iter()
        .map(|a| {
            let tp = a.tipo_ativo.to_lowercase();
            let is_short = short_keywords
                .iter()
                .any(|kw| tp.contains(kw));
            if is_short {
                (a.vl_porcentagem_pl, 0.0)
            } else {
                (0.0, a.vl_porcentagem_pl)
            }
        })
        .fold((0.0, 0.0), |(s, l), (ds, dl)| (s + ds, l + dl));

    if short == 0.0 && long == 0.0 {
        (50.0, 50.0)
    } else {
        (short, long)
    }
}

fn draw_kpi_card(
    ui: &mut egui::Ui,
    title: &str,
    value: &str,
    description: &str,
    accent_color: egui::Color32,
    card_bg: egui::Color32,
) {
    let (text_color, desc_color) = if ui.visuals().dark_mode {
        (
            egui::Color32::WHITE,
            egui::Color32::from_rgb(180, 190, 205),
        )
    } else {
        (
            egui::Color32::from_rgb(20, 25, 35),
            egui::Color32::from_rgb(70, 75, 90),
        )
    };

    egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .fill(card_bg)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_height(70.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let (rect, _response) =
                    ui.allocate_exact_size(egui::vec2(4.0, 54.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(2), accent_color);

                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(title)
                            .size(Scale::DEFAULT.label_mini())
                            .strong()
                            .color(desc_color),
                    );
                    ui.label(
                        egui::RichText::new(value)
                            .size(Scale::DEFAULT.metric())
                            .strong()
                            .color(text_color),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(description)
                            .size(Scale::DEFAULT.description())
                            .color(desc_color),
                    );
                });
            });
        });
}
