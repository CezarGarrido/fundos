use crate::provider::{self};
use chrono::NaiveDate;
use polars::frame::DataFrame;

pub enum Message {
    StartDownload,
    SearchFunds(String, Option<provider::cvm::fund::Class>),
    ResultFunds(DataFrame),
    NewTab(String),
    Profit(String, NaiveDate, NaiveDate),
    Assets(String, String, String),
    ProfitResult(String, DataFrame, DataFrame, DataFrame),
    AssetsResult(String, DataFrame, DataFrame, DataFrame, String, String),
    OpenSearchWindow(bool),
    ShowAssetDetail(DataFrame),
    OpenDashboardTab,
    DashboardTabResult(DataFrame, DataFrame, DataFrame),
    OpenTab(String, DataFrame),
    OpenAtivosTab(NaiveDate, NaiveDate),
    AtivosTabResult(DataFrame),
    OpenHistoricoTab(String, NaiveDate, NaiveDate),
    HistoricoTabResult(String, DataFrame),
    FetchYahooPrice(String, String, NaiveDate, NaiveDate),
    YahooPriceResult(String, DataFrame),
    FetchAssetHolders(String, NaiveDate, NaiveDate),
    AssetHoldersResult(String, DataFrame),
    HistoricoSeriesResult(String, Vec<crate::ui::fund::tab::historico::MonthlySeries>),
}
