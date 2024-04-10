use cmd::{add::add, check::check, del::del};
use rscni::{async_skel::Plugin, version::PluginInfo};
use version::{CNI_VERSION, SUPPORTED_VERSIONS};

mod cmd;
mod config;
mod error;
mod mock;
mod proto;
mod version;

#[tokio::main]
async fn main() {
    let version_info = PluginInfo::new(
        CNI_VERSION,
        SUPPORTED_VERSIONS
            .to_vec()
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<String>>(),
    );
    let mut plugin = Plugin::new(add, del, check, version_info, &get_about_info());

    match plugin.run().await {
        Ok(_) => std::process::exit(0),
        Err(e) => std::process::exit(1),
    }
}

fn get_about_info() -> String {
    format!(
        "Sart CNI plugin",
    )
}
