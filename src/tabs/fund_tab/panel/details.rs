use egui::{Frame, Ui};
use egui_extras::{Column, TableBuilder};
use polars::frame::DataFrame;
use std::collections::HashMap;

pub fn show_ui(df: DataFrame, ui: &mut Ui) {
    ui.group(|ui| {
        ui.label(format!("{} Detalhes", egui_phosphor::regular::NOTE));
        ui.separator();
        Frame::none().inner_margin(5.0).show(ui, |ui| {
            ui.push_id("dados_id", |ui| {
                show_dataframe(
                    vec!["TP_FUNDO", "CNPJ_FUNDO", "DT_REG", "DT_CONST", "CD_CVM"],
                    df.clone(),
                    ui,
                );
            });
            ui.separator();

            ui.push_id("classe_id", |ui| {
                show_dataframe(vec!["CLASSE", "CLASSE_ANBIMA"], df.clone(), ui);
            });
            ui.separator();
            ui.push_id("situacao_id", |ui| {
                show_dataframe(
                    vec!["SIT", "DT_INI_SIT", "DT_INI_ATIV", "DT_CANCEL"],
                    df.clone(),
                    ui,
                );
            });
            ui.separator();
            ui.push_id("admin_id", |ui| {
                show_dataframe(
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
                    df,
                    ui,
                );
            });
        });
    });
}

fn show_dataframe(columns: Vec<&str>, df: DataFrame, ui: &mut Ui) {
    let n_rows = df.height();
    let cols = df.get_columns();
    ui.horizontal(|ui| {
        ui.set_width(ui.available_width());
        ui.set_height(ui.available_height());
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::auto().resizable(false))
            .column(Column::remainder())
            .body(|mut body| {
                for row in 0..n_rows {
                    for col in cols.iter() {
                        let field_name = col.name();
                        if !columns.contains(&field_name) {
                            continue;
                        }
                        body.row(18.0, |mut row_ui| {
                            row_ui.col(|ui| {
                                ui.label(&format!("{}", header_title(field_name)));
                            });
                            row_ui.col(|ui| {
                                let value = col.get(row).unwrap();
                                if let Some(value_str) = value.get_str() {
                                    ui.label(format!("{}", value_str));
                                } else {
                                    ui.label("-");
                                }
                            });
                        });
                    }
                }
            });
    });
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
