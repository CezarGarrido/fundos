use crate::ui::download::Download;

pub mod cdi;
pub mod ibovespa;

pub fn download(
    token: tokio_util::sync::CancellationToken,
    name: String,
    on_progress: impl 'static + Send + FnMut(Download),
) {
    if name == "CDI" {
        return cdi::download(token, on_progress);
    }

    if name == "IBOV" {
        ibovespa::download(token, on_progress);
    }
}
