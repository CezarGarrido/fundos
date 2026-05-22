use crate::util;
use egui::Ui;
use polars::prelude::*;

pub fn show_kpis(assets: DataFrame, pl: DataFrame, ui: &mut Ui) {
    // Definir paleta de cores dinâmicas de alto contraste de acordo com o tema
    let (heading_color, _text_color, secondary_color, success_color, card_bg) =
        if ui.visuals().dark_mode {
            (
                egui::Color32::from_rgb(255, 255, 255), // Branco brilhante para títulos
                egui::Color32::from_rgb(230, 235, 245), // Cinza muito claro para leitura
                egui::Color32::from_rgb(175, 185, 200), // Cinza intermediário para secundários (legível)
                egui::Color32::from_rgb(46, 204, 113),  // Verde esmeralda
                egui::Color32::from_rgb(30, 35, 45),    // Fundo do card escuro
            )
        } else {
            (
                egui::Color32::from_rgb(10, 15, 25), // Azul escuro/preto para títulos
                egui::Color32::from_rgb(30, 35, 45), // Cinza grafite escuro para leitura
                egui::Color32::from_rgb(80, 85, 100), // Cinza escuro legível para secundários
                egui::Color32::from_rgb(19, 115, 51), // Verde escuro floresta
                egui::Color32::from_rgb(245, 247, 250), // Fundo do card claro
            )
        };

    if assets.height() == 0 {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new(egui_phosphor::regular::LIGHTBULB.to_string())
                    .color(egui::Color32::from_rgb(250, 185, 80))
                    .size(48.0),
            );
            ui.add_space(15.0);
            ui.heading(
                egui::RichText::new("Aguardando Dados da Carteira")
                    .color(heading_color)
                    .size(18.0)
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Por favor, selecione e carregue uma data de referência na aba 'Carteira' primeiro para gerar os insights automáticos da carteira.")
                    .color(secondary_color)
                    .size(13.0)
            );
        });
        return;
    }

    // Obter estatísticas da maior posição
    let max_pos = get_largest_position(&assets);
    let total_assets_count = get_unique_assets_count(&assets);
    let (_short_term_pct, _long_term_pct) = get_duration_split(&assets);

    // Grid Superior: Resumos Rápidos (KPI Cards)
    ui.columns(3, |cols| {
        // Card 1: Diversificação
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

        // Card 2: Maior Posição %
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

        // Card 3: Patrimônio Líquido
        let pl_val = get_pl_value(&pl);
        let pl_formatted = if pl_val > 0.0 {
            util::to_real(pl_val).unwrap().format()
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
    assets: DataFrame,
    _pl: DataFrame,
    fund_history: Option<DataFrame>,
    ui: &mut Ui,
) {
    if assets.height() == 0 {
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

    let max_pos = get_largest_position(&assets);
    let (short_term_pct, long_term_pct) = get_duration_split(&assets);

    ui.vertical(|ui| {
        // Bloco Superior: Posição de Convicção e PM
        ui.vertical(|ui| {
            ui.heading(
                        egui::RichText::new(format!(
                            "{} Posição de Maior Convicção",
                            egui_phosphor::regular::TARGET
                        ))
                        .size(14.0)
                        .strong()
                        .color(heading_color)
                    );
                    ui.separator();
                    ui.add_space(8.0);

                    if let Some(pos) = &max_pos {
                        ui.label(
                            egui::RichText::new(&pos.name)
                                .size(13.0)
                                .strong()
                                .color(ui.visuals().selection.bg_fill)
                        );
                        ui.add_space(8.0);

                        egui::Grid::new("largest_pos_grid")
                            .striped(true)
                            .num_columns(2)
                            .min_col_width(ui.available_width() / 2.0 - 5.0)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Exposição Absoluta:").color(text_color));
                                if let Ok(formatted) = util::to_real(pos.value) {
                                    ui.label(egui::RichText::new(formatted.format()).strong().color(text_color));
                                } else {
                                    ui.label(egui::RichText::new("R$ -").color(text_color));
                                }
                                ui.end_row();

                                ui.label(egui::RichText::new("Exposição no PL:").color(text_color));
                                ui.label(egui::RichText::new(format!("{:.3}%", pos.pct)).strong().color(success_color));
                                ui.end_row();

                                ui.label(egui::RichText::new("Tipo de Ativo:").color(text_color));
                                ui.label(egui::RichText::new(&pos.tp_aplic).color(secondary_color));
                                ui.end_row();

                                if pos.cost > 0.0 {
                                    ui.label(egui::RichText::new("Custo de Aquisição:").color(text_color));
                                    if let Ok(formatted) = util::to_real(pos.cost) {
                                        ui.label(egui::RichText::new(formatted.format()).color(text_color));
                                    } else {
                                        ui.label(egui::RichText::new("N/A").color(text_color));
                                    }
                                    ui.end_row();

                                    // Cálculo de Ganho/Perda para o Gestor na posição
                                    let profit_pct = ((pos.value / pos.cost) - 1.0) * 100.0;
                                    let profit_value = pos.value - pos.cost;
                                    ui.label(egui::RichText::new("Resultado Acumulado:").color(text_color));
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
                                    if let Ok(formatted) = util::to_real(profit_value.abs()) {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} R$ {} ({:+.2}%)",
                                                icon,
                                                formatted.format().replace("R$", ""),
                                                profit_pct
                                            ))
                                            .strong()
                                            .color(color),
                                        );
                                    } else {
                                        ui.label(egui::RichText::new("N/A").color(text_color));
                                    }
                                    ui.end_row();
                                }
                            });

                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(15.0);
                        // Mensagem de análise de risco e regulação
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(egui_phosphor::regular::SHIELD_CHECK.to_string()).color(success_color));
                                ui.label(egui::RichText::new("Análise de Risco de Liquidez").strong().size(12.0).color(heading_color));
                            });
                            ui.separator();
                            let msg = if pos.pct > 20.0 {
                                "ATENÇÃO: A alocação neste ativo excede 20% do PL total do fundo. Trata-se de uma exposição de altíssimo risco e altíssima dependência do emissor, geralmente justificável apenas em fundos com estratégia exclusiva ou títulos de dívida soberana federal."
                            } else if pos.pct > 10.0 {
                                "ALERTA: Posição acima de 10%. Para emissores corporativos privados convencionais, a alocação de um único ativo/emissor é regulamentada até o teto de 10% (CVM 175) para evitar contágio. Confirme se o ativo refere-se a Títulos Públicos Federais ou se possui isenção estrutural."
                            } else {
                                "CONCORDÂNCIA: A maior posição está perfeitamente enquadrada abaixo do limite prudencial padrão de 10% por ativo/emissor privado. Gestão adota um modelo equilibrado de diversificação."
                            };
                            ui.label(egui::RichText::new(msg).size(11.0).color(text_color));
                        });

                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(15.0);

                        // RF11 & Analytics: Comportamento do Gestor
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(egui_phosphor::regular::BRAIN.to_string()).color(egui::Color32::from_rgb(155, 89, 182)));
                                ui.label(egui::RichText::new("Comportamento do Gestor (Analytics)").strong().size(12.0).color(heading_color));
                            });
                            ui.separator();

                            if let Some(history_df) = &fund_history {
                                if let Some(analytics) = crate::analytics::compute_asset_analytics(history_df, &pos.codigo, None) {
                                    egui::Grid::new("analytics_grid")
                                        .striped(true)
                                        .num_columns(2)
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new("Inércia (Montagem):").color(text_color));
                                            ui.label(egui::RichText::new(format!("{} meses", analytics.speed_to_peak)).strong().color(secondary_color));
                                            ui.end_row();

                                            ui.label(egui::RichText::new("PM Compra Estimado:").color(text_color));
                                            if let Ok(formatted) = util::to_real(analytics.avg_buy_price) {
                                                ui.label(egui::RichText::new(formatted.format()).color(success_color));
                                            }
                                            ui.end_row();

                                            ui.label(egui::RichText::new("Take-Profit Gatilho:").color(text_color));
                                            let tp_color = if analytics.take_profit_trigger > 0.0 { success_color } else { secondary_color };
                                            ui.label(egui::RichText::new(format!("{:.1}% do PL", analytics.take_profit_trigger)).color(tp_color));
                                            ui.end_row();

                                            if !analytics.hidden_qty_estimates.is_empty() {
                                                // ── Análise de Integridade da Carteira ────
                                                if let Some(ref inf) = analytics.portfolio_inference {
                                                    let score_color = if inf.stability_score > 0.75 {
                                                        success_color
                                                    } else if inf.stability_score > 0.40 {
                                                        egui::Color32::from_rgb(234, 179, 8)
                                                    } else if inf.stability_score > 0.15 {
                                                        egui::Color32::from_rgb(249, 115, 22)
                                                    } else {
                                                        egui::Color32::from_rgb(231, 76, 60)
                                                    };
                                                    ui.label(egui::RichText::new("Inferência:").color(text_color));
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "Estab {:.0}%  Z={:+.1}  {}",
                                                            inf.stability_score * 100.0,
                                                            inf.z_score,
                                                            inf.bias_direction
                                                        ))
                                                        .strong()
                                                        .color(score_color),
                                                    );
                                                    ui.end_row();
                                                }
                                                ui.label(egui::RichText::new("Posição Oculta (Estimada):").color(text_color));
                                                if let Some(last_est) = analytics.hidden_qty_estimates.last() {
                                                    let warn_color = if ui.visuals().dark_mode {
                                                        egui::Color32::from_rgb(230, 126, 34)
                                                    } else {
                                                        egui::Color32::from_rgb(190, 95, 10)
                                                    };
                                                    let val_str = crate::util::to_real(last_est.2)
                                                        .map(|r| r.format())
                                                        .unwrap_or_else(|_| format!("R$ {:.2}", last_est.2));
                                                    ui.label(
                                                        egui::RichText::new(format!(
                                                            "{:.0} cotas ≈ {} em {}",
                                                            last_est.1, val_str, last_est.0
                                                        ))
                                                        .strong()
                                                        .color(warn_color),
                                                    );
                                                }
                                                ui.end_row();
                                            }
                                        });
                                } else {
                                    let is_generic = pos.codigo.len() < 4 || pos.codigo.contains("Ações") || pos.codigo.contains("Operações");
                                    if is_generic {
                                        ui.label(egui::RichText::new(format!("A maior exposição ('{}') é uma classe genérica ou está sob sigilo recente da CVM. Não é possível traçar o perfil do ativo.", pos.name)).size(11.0).color(secondary_color));
                                    } else {
                                        ui.label(egui::RichText::new("Dados históricos insuficientes para calcular análise comportamental deste ativo.").size(11.0).color(secondary_color));
                                    }
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Carregando base histórica do fundo para análise...").size(11.0).color(secondary_color));
                                    ui.spinner();
                                });
                            }
                        });
                    } else {
                        ui.label(egui::RichText::new("Sem dados específicos de ativos na carteira.").color(secondary_color));
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
                        .size(14.0)
                        .strong()
                        .color(heading_color)
                    );
                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(egui::RichText::new("Perfil Estrutural de Liquidez:").strong().color(heading_color));
                    ui.add_space(10.0);

                    // Desenhar barra de progresso horizontal comparando Curto vs Longo Prazo
                    let total_pct = short_term_pct + long_term_pct;
                    let short_ratio = if total_pct > 0.0 { short_term_pct / total_pct } else { 0.5 };

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Liquidez / Curto Prazo").size(11.0).color(secondary_color));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("Estratégico / Longo Prazo").size(11.0).color(secondary_color));
                        });
                    });

                    // Barra customizada
                    let (rect, _response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 16.0), egui::Sense::hover());
                    let painter = ui.painter();

                    // Fundo da barra (Longo Prazo - Laranja de alto contraste)
                    let orange_color = if ui.visuals().dark_mode {
                        egui::Color32::from_rgb(230, 126, 34)
                    } else {
                        egui::Color32::from_rgb(190, 95, 10)
                    };
                    painter.rect_filled(rect, egui::CornerRadius::same(4), orange_color);

                    // Parte Curta da barra (Curto Prazo - Verde de alto contraste)
                    let short_width = rect.width() * short_ratio as f32;
                    let short_rect = egui::Rect::from_min_size(rect.min, egui::vec2(short_width, rect.height()));
                    painter.rect_filled(short_rect, egui::CornerRadius::same(4), success_color);

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("● {:.1}%", short_term_pct))
                                .color(success_color)
                                .strong()
                                .size(12.0),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("● {:.1}%", long_term_pct))
                                    .color(orange_color)
                                    .strong()
                                    .size(12.0),
                            );
                        });
                    });

                    ui.add_space(20.0);
                    ui.label(egui::RichText::new("Interpretação da Duration / Liquidez:").strong().size(12.0).color(heading_color));
                    ui.separator();
                    let profile_msg = if short_term_pct > 70.0 {
                        "CARTEIRA ULTRA-LÍQUIDA: Mais de 70% da carteira está alocada em instrumentos de alta liquidez e curtíssimo prazo (como Títulos Públicos federais Selic e caixa imediato). Este perfil apresenta volatilidade mínima, protegendo o cotista de riscos de taxas de juros de mercado futuras."
                    } else if long_term_pct > 70.0 {
                        "CARTEIRA DE VALOR / LONGO PRAZO: Mais de 70% da carteira foca em ativos estruturais de longo prazo (ações, debêntures, títulos corporativos longos). Esta carteira está fortemente exposta a riscos de volatilidade do mercado acionário e variação de taxas de juros futuras (marcação a mercado ativa), indicada para horizontes de investimento longos (> 36 meses)."
                    } else {
                        "PERFIL EQUILIBRADO / HÍBRIDO: O fundo equilibra o caixa e liquidez rápida (cerca de metade da carteira) com investimentos estruturais de longo prazo para buscar Alfa sem comprometer as janelas de resgate rápidas."
                    };
                    ui.label(egui::RichText::new(profile_msg).size(11.0).color(text_color));
            // Fim Coluna Direita
                });
    });
}

struct LargestPos {
    name: String,
    codigo: String,
    pct: f64,
    value: f64,
    cost: f64,
    tp_aplic: String,
}

fn get_largest_position(assets: &DataFrame) -> Option<LargestPos> {
    if assets.height() == 0 {
        return None;
    }

    let pct_col = assets.column("VL_PORCENTAGEM_PL").ok()?;
    let mut max_idx = 0;
    let mut max_val = -1.0;

    for i in 0..pct_col.len() {
        if let Ok(val) = pct_col.get(i) {
            let val_f64 = val.try_extract::<f64>().unwrap_or(0.0);
            if val_f64 > max_val {
                max_val = val_f64;
                max_idx = i;
            }
        }
    }

    if max_val <= 0.0 {
        return None;
    }

    // Identificar o nome do ativo buscando colunas válidas de identificação
    let name = [
        "CD_ATIVO",
        "DS_ATIVO",
        "NM_FUNDO_COTA",
        "TP_TITPUB",
        "CD_SELIC",
        "TP_APLIC",
    ]
    .iter()
    .filter_map(|&col| {
        assets
            .column(col)
            .ok()
            .and_then(|c| c.get(max_idx).ok())
            .and_then(|v| v.get_str().map(|s| s.to_string()))
    })
    .next()
    .unwrap_or_else(|| "Ativo Não Nomeado".to_string());

    let codigo = assets
        .column("CD_ATIVO")
        .or_else(|_| assets.column("CD_ISIN"))
        .ok()
        .and_then(|c| c.get(max_idx).ok())
        .and_then(|v| v.get_str().map(|s| s.to_string()))
        .unwrap_or_else(|| name.clone());

    let value = assets
        .column("VL_MERC_POS_FINAL")
        .ok()
        .and_then(|c| c.get(max_idx).ok())
        .and_then(|v| {
            v.try_extract::<f64>().ok().or_else(|| {
                v.get_str()
                    .and_then(|s| s.replace(',', ".").parse::<f64>().ok())
            })
        })
        .unwrap_or(0.0);

    let cost = assets
        .column("VL_CUSTO_POS_FINAL")
        .ok()
        .and_then(|c| c.get(max_idx).ok())
        .and_then(|v| {
            v.try_extract::<f64>().ok().or_else(|| {
                v.get_str()
                    .and_then(|s| s.replace(',', ".").parse::<f64>().ok())
            })
        })
        .unwrap_or(0.0);

    let tp_aplic = assets
        .column("TP_APLIC")
        .ok()
        .and_then(|c| c.get(max_idx).ok())
        .and_then(|v| v.get_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "N/A".to_string());

    Some(LargestPos {
        name,
        codigo,
        pct: max_val,
        value,
        cost,
        tp_aplic,
    })
}

fn get_unique_assets_count(assets: &DataFrame) -> usize {
    if assets.height() == 0 {
        return 0;
    }

    if let Ok(col) = assets.column("CD_ATIVO") {
        let mut set = std::collections::HashSet::new();
        for i in 0..col.len() {
            if let Ok(val) = col.get(i) {
                if let Some(s) = val.get_str() {
                    set.insert(s.to_string());
                }
            }
        }
        if !set.is_empty() {
            return set.len();
        }
    }

    assets.height()
}

fn get_pl_value(pl: &DataFrame) -> f64 {
    if pl.height() == 0 {
        return 0.0;
    }

    pl.column("VL_PATRIM_LIQ")
        .ok()
        .and_then(|c| c.get(0).ok())
        .and_then(|v| {
            v.try_extract::<f64>().ok().or_else(|| {
                v.get_str()
                    .and_then(|s| s.replace(',', ".").parse::<f64>().ok())
            })
        })
        .unwrap_or(0.0)
}

fn get_duration_split(assets: &DataFrame) -> (f64, f64) {
    if assets.height() == 0 {
        return (0.0, 0.0);
    }

    let mut short_term = 0.0;
    let mut long_term = 0.0;

    let tp_aplic_col = assets.column("TP_APLIC").ok();
    let pct_col = assets.column("VL_PORCENTAGEM_PL").ok();

    if let (Some(tp_col), Some(p_col)) = (tp_aplic_col, pct_col) {
        for i in 0..assets.height() {
            let tp_val = tp_col
                .get(i)
                .ok()
                .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                .unwrap_or_default();
            let pct_val = p_col
                .get(i)
                .ok()
                .and_then(|v| {
                    v.try_extract::<f64>().ok().or_else(|| {
                        v.get_str()
                            .and_then(|s| s.replace(',', ".").parse::<f64>().ok())
                    })
                })
                .unwrap_or(0.0);

            let is_short = tp_val.contains("títulos públicos")
                || tp_val.contains("operações compromissadas")
                || tp_val.contains("caixa")
                || tp_val.contains("disponibilidades")
                || tp_val.contains("renda fixa");

            if is_short {
                short_term += pct_val;
            } else {
                long_term += pct_val;
            }
        }
    }

    if short_term == 0.0 && long_term == 0.0 {
        (50.0, 50.0)
    } else {
        (short_term, long_term)
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
        (egui::Color32::WHITE, egui::Color32::from_rgb(180, 190, 205))
    } else {
        (
            egui::Color32::from_rgb(20, 25, 35),
            egui::Color32::from_rgb(70, 75, 90),
        )
    };

    egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .fill(card_bg) // Fundo customizado do card para alto contraste
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_height(70.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                // Barra vertical colorida
                let (rect, _response) =
                    ui.allocate_exact_size(egui::vec2(4.0, 54.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(2), accent_color);

                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(title)
                            .size(9.0)
                            .strong()
                            .color(desc_color),
                    );
                    ui.label(
                        egui::RichText::new(value)
                            .size(17.0)
                            .strong()
                            .color(text_color),
                    );
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(description).size(9.5).color(desc_color));
                });
            });
        });
}
