use egui::{Frame, Grid, Ui};
use polars::prelude::*;
use std::collections::HashMap;

pub fn show_ui(df: DataFrame, ui: &mut Ui) {
    ui.group(|ui| {
        ui.heading(format!("{} Detalhes", egui_phosphor::regular::NOTE));
        ui.separator();
        Frame::none().inner_margin(0.0).show(ui, |ui| {
            let grouped_columns = get_grouped_columns(&df);
            show_dataframe(grouped_columns, df.clone(), ui);
        });
    });
}

fn get_grouped_columns(df: &DataFrame) -> Vec<(&str, Vec<&str>)> {
    let predefined_groups = vec![
        (
            "dados_id",
            vec!["TP_FUNDO", "CNPJ_FUNDO", "DT_REG", "DT_CONST", "CD_CVM"],
        ),
        ("classe_id", vec!["CLASSE", "CLASSE_ANBIMA"]),
        (
            "situacao_id",
            vec!["SIT", "DT_INI_SIT", "DT_INI_ATIV", "DT_CANCEL"],
        ),
        (
            "admin_id",
            vec![
                "DIRETOR",
                "ADMIN",
                "CNPJ_ADMIN",
                "GESTOR",
                "CPF_CNPJ_GESTOR",
                "PF_PJ_GESTOR",
                "AUDITOR",
                "CNPJ_AUDITOR",
                "CUSTODIANTE",
            ],
        ),
    ];

    let grouped_column_names: Vec<&str> = predefined_groups
        .iter()
        .flat_map(|(_, cols)| cols.iter().copied())
        .collect();

    let all_columns: Vec<&str> = df
        .get_columns()
        .iter()
        .map(|col| col.name())
        .filter(|col_name| !grouped_column_names.contains(col_name))
        .collect();

    let mut extended_grouped_columns = predefined_groups;
    extended_grouped_columns.push(("all_id", all_columns));
    extended_grouped_columns
}

fn show_dataframe(grouped_columns: Vec<(&str, Vec<&str>)>, df: DataFrame, ui: &mut Ui) {
    let n_rows = df.height();
    let cols = df.get_columns();

    egui::ScrollArea::vertical().show(ui, |ui| {
        Grid::new("data_grid")
            .striped(true)
            .min_col_width(ui.available_width() / 2.0)
            .max_col_width(ui.available_width() / 2.0)
            .show(ui, |ui| {
                for (group_id, columns) in &grouped_columns {
                    if group_id != &"all_id" {
                        show_columns(group_id, columns, cols, n_rows, ui);
                    }
                }
            });

        let all_columns = grouped_columns
            .iter()
            .find_map(|(group_id, cols)| {
                if group_id == &"all_id" {
                    Some(cols.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        ui.collapsing("Ver mais", |ui| {
            Grid::new("all_data_grid")
                .striped(true)
                .max_col_width(ui.available_width() / 2.0)
                .min_col_width(ui.available_width() / 2.0)
                .show(ui, |ui| {
                    show_columns("all_id", &all_columns, cols, n_rows, ui);
                });
        });
    });
}

fn show_columns(_group_id: &str, columns: &[&str], cols: &[Series], n_rows: usize, ui: &mut Ui) {
    for row in 0..n_rows {
        for col in cols.iter() {
            let field_name = col.name();
            if !columns.contains(&field_name) {
                continue;
            }
            ui.label(header_title(field_name));
            let value = col.get(row).unwrap();

            if let Some(value_str) = value.get_str() {
                ui.label(value_str.to_string());
            } else {
                ui.label(format!("{:#}", value));
            }
            ui.end_row();
        }
    }
}
fn header_title(header: &str) -> String {
    let mut headers_map = HashMap::new();
    headers_map.insert("TP_FUNDO", "Tipo de Fundo");
    headers_map.insert("CNPJ_FUNDO", "CNPJ do Fundo");
    headers_map.insert("DENOM_SOCIAL", "Denominação Social");
    headers_map.insert("DT_REG", "Data de Registro");
    headers_map.insert("DT_CONST", "Data de Constituição");
    headers_map.insert("CD_CVM", "Código CVM");
    headers_map.insert("DT_CANCEL", "Data de Cancelamento");
    headers_map.insert("SIT", "Situação");
    headers_map.insert("DT_INI_SIT", "Data de Início da Situação");
    headers_map.insert("DT_INI_ATIV", "Data de Início da Atividade");
    headers_map.insert("DT_INI_EXERC", "Data de Início do Exercício");
    headers_map.insert("DT_FIM_EXERC", "Data de Fim do Exercício");
    headers_map.insert("CLASSE", "Classe");
    headers_map.insert("DT_INI_CLASSE", "Data de Início da Classe");
    headers_map.insert("RENTAB_FUNDO", "Rentabilidade do Fundo");
    headers_map.insert("CONDOM", "Condomínio");
    headers_map.insert("FUNDO_COTAS", "Fundo de Cotas");
    headers_map.insert("FUNDO_EXCLUSIVO", "Fundo Exclusivo");
    headers_map.insert("TRIB_LPRAZO", "Tributação de Longo Prazo");
    headers_map.insert("PUBLICO_ALVO", "Público Alvo");
    headers_map.insert("ENTID_INVEST", "Entidade de Investimento");
    headers_map.insert("TAXA_PERFM", "Taxa de Performance");
    headers_map.insert("INF_TAXA_PERFM", "Informação sobre a Taxa de Performance");
    headers_map.insert("TAXA_ADM", "Taxa de Administração");
    headers_map.insert("INF_TAXA_ADM", "Informação sobre a Taxa de Administração");
    headers_map.insert("VL_PATRIM_LIQ", "Valor Patrimonial Líquido");
    headers_map.insert("DT_PATRIM_LIQ", "Data do Valor Patrimonial Líquido");
    headers_map.insert("DIRETOR", "Diretor");
    headers_map.insert("CNPJ_ADMIN", "CNPJ do Administrador");
    headers_map.insert("ADMIN", "Administrador");
    headers_map.insert("PF_PJ_GESTOR", "Pessoa Física ou Jurídica do Gestor");
    headers_map.insert("CPF_CNPJ_GESTOR", "CPF ou CNPJ do Gestor");
    headers_map.insert("GESTOR", "Gestor");
    headers_map.insert("CNPJ_AUDITOR", "CNPJ do Auditor");
    headers_map.insert("AUDITOR", "Auditor");
    headers_map.insert("CNPJ_CUSTODIANTE", "CNPJ do Custodiante");
    headers_map.insert("CUSTODIANTE", "Custodiante");
    headers_map.insert("CNPJ_CONTROLADOR", "CNPJ do Controlador");
    headers_map.insert("CONTROLADOR", "Controlador");
    headers_map.insert("INVEST_CEMPR_EXTER", "Investimento em Empresas no Exterior");
    headers_map.insert("CLASSE_ANBIMA", "Classe ANBIMA");

    match headers_map.get(header) {
        Some(humanized_name) => humanized_name.to_string(),
        None => String::from(header),
    }
}
