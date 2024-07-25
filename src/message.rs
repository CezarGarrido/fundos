use chrono::NaiveDate;
use polars::frame::DataFrame;

use crate::cvm;

pub enum Message {
    SearchFunds(String, Option<cvm::fund::Class>),
    ResultFunds(DataFrame),
    NewTab(String),
    Profit(String, NaiveDate, NaiveDate),
    Assets(String, String, String),
    ProfitResult(String, DataFrame, DataFrame),
    AssetsResult(String, DataFrame, DataFrame, DataFrame),
    DownloadMessage(usize, String),
    OpenSearchWindow(bool),
}
