use chrono::NaiveDate;
use polars::frame::DataFrame;

use crate::{
    provider,
    ui::download::{Download, DownloadItem},
};

pub enum Message {
    SearchFunds(String, Option<provider::cvm::fund::Class>),
    ResultFunds(DataFrame),
    NewTab(String),
    Profit(String, NaiveDate, NaiveDate),
    Assets(String, String, String),
    ProfitResult(String, DataFrame, DataFrame, DataFrame),
    AssetsResult(String, DataFrame, DataFrame, DataFrame),
    OpenSearchWindow(bool),
    StartDownload(String, usize, DownloadItem),
    CancelDownload(String),
    ProgressDownload(String, usize, Download),
    ShowAssetDetail(DataFrame),
    OpenDashboardTab,
    DashboardTabResult(DataFrame, DataFrame, DataFrame),
    OpenConfigWindow(bool),
}
