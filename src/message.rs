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
    AssetsResult(String, DataFrame, DataFrame, DataFrame),
    OpenSearchWindow(bool),
    ShowAssetDetail(DataFrame),
    OpenDashboardTab,
    DashboardTabResult(DataFrame, DataFrame, DataFrame),
    OpenTab(String, DataFrame),
}
