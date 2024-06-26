use anyhow::Result;
use clap::Parser;
use std::{io::Write, path::Path};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long = "bin-dir", default_value = DEFAULT_BIN_DIR)]
    bin_dir: String,

    #[arg(long = "conf-dir", default_value = DEFAULT_CONF_DIR)]
    conf_dir: String,

    #[arg(long = "src-bin-dir", default_value = DEFAULT_SRC_BIN_DIR)]
    src_bin_dir: String,

    #[arg(long = "src-conf-dir", default_value = DEFAULT_SRC_CONF_DIR)]
    src_conf_dir: String,

    #[arg(long)]
    binaries: Option<Vec<String>>,
}

const BIN_NAME: &str = "sart-cni";

const DEFAULT_SRC_BIN_DIR: &str = "/host/opt/cni/bin";
const DEFAULT_SRC_CONF_DIR: &str = "/host/etc/cni/net.d";

const DEFAULT_BIN_DIR: &str = "/opt/cni/bin";
const DEFAULT_CONF_DIR: &str = "/etc/cni/net.d";
const CNI_CONF_ENV_KEY: &str = "CNI_NETCONF";
const CNI_CONF_NAME: &str = "10-netconf.conflist";

fn main() -> Result<()> {
    println!("Install CNI binary and configuration file");
    let arg = Args::parse();

    let binaries = match arg.binaries {
        Some(b) => b.clone(),
        None => vec![BIN_NAME.to_string()],
    };

    install_cni_binaries(&binaries, &arg.src_bin_dir, &arg.bin_dir)?;
    if std::env::var(CNI_CONF_ENV_KEY).is_ok() {
        install_cni_conf_from_env(CNI_CONF_ENV_KEY, &arg.conf_dir)?;
    } else {
        install_cni_conf(&arg.src_conf_dir, &arg.conf_dir)?;
    }

    Ok(())
}

fn install_cni_binaries(binaries: &[String], src_dir: &str, dst_dir: &str) -> Result<()> {
    std::fs::create_dir_all(dst_dir)?;

    let src = Path::new(src_dir);
    let dst = Path::new(dst_dir);
    for binary in binaries.iter() {
        let src_path = src.join(binary);
        let dst_path = dst.join(binary);
        std::fs::copy(src_path, dst_path)?;
    }
    Ok(())
}

fn install_cni_conf(src: &str, dst: &str) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    // clean up existing conf
    let files = std::fs::read_dir(dst)?;
    for file in files.into_iter() {
        let file = file?;
        std::fs::remove_file(file.path())?;
    }

    let dst_dir = Path::new(dst);
    let new_files = std::fs::read_dir(src)?;
    for file in new_files.into_iter() {
        let file = file?;
        std::fs::copy(file.path(), dst_dir.join(file.file_name()))?;
    }

    Ok(())
}

fn install_cni_conf_from_env(key: &str, dst: &str) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    // clean up existing conf
    let files = std::fs::read_dir(dst)?;
    for file in files.into_iter() {
        let file = file?;
        std::fs::remove_file(file.path())?;
    }

    let dst_dir = Path::new(dst);
    let conf = std::env::var(key)?;

    let mut file = std::fs::File::create(dst_dir.join(CNI_CONF_NAME))?;
    file.write_all(conf.as_bytes())?;

    Ok(())
}
