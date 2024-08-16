use chrono::NaiveDate;
use polars::frame::DataFrame;

use crate::provider::{
    self,
    downloader::{DownloadItem, DownloadStatus},
};

pub enum Message {
    RefreshConfig,
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
    ProgressDownload(String, usize, DownloadStatus),
    ShowAssetDetail(DataFrame),
    OpenDashboardTab,
    DashboardTabResult(DataFrame, DataFrame, DataFrame),
    UpdateStatus(String),
}
