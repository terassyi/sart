use std::{
    fs::File,
    os::{
        fd::{AsFd, AsRawFd},
        unix::fs::MetadataExt,
    },
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::netlink;

const NETNS_PATH_BASE: &str = "/var/run/netns";

#[derive(Debug)]
pub struct NetNS {
    path: PathBuf,
    file: File,
}

impl NetNS {
    pub fn new(name: &str) -> NetNS {
        let base = Path::new(NETNS_PATH_BASE);
        let file = std::fs::File::create(base).unwrap();
        NetNS {
            path: base.join(name),
            file,
        }
    }

    pub fn enter(&self) -> Result<(), Error> {
        nix::sched::setns(self.file.as_fd(), nix::sched::CloneFlags::CLONE_NEWNET)
            .map_err(Error::SetNS)
    }

    pub fn run<F, T>(&self, f: F) -> Result<T, Error>
    where
        F: FnOnce(&Self) -> T,
    {
        let src = get_current_netns()?;
        if src.path.eq(&self.path) {
            return Ok(f(self));
        }

        self.enter()?;

        let res = f(self);

        src.enter()?;

        Ok(res)
    }

    pub fn fd(&self) -> i32 {
        self.file.as_raw_fd()
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

pub fn get_current_netns() -> Result<NetNS, Error> {
    let path = get_current_netns_path();
    let file = std::fs::File::open(&path).map_err(|e| Error::OpenNetNS(path.clone(), e))?;
    Ok(NetNS { path, file })
}

fn get_current_netns_path() -> PathBuf {
    let id = nix::unistd::gettid();
    PathBuf::from(format!("/proc/self/task/{}/ns/net", id))
}

impl PartialEq for NetNS {
    fn eq(&self, other: &Self) -> bool {
        if self.file.as_raw_fd() == other.file.as_raw_fd() {
            return true;
        }
        let cmp_meta = |f1: &File, f2: &File| -> Option<bool> {
            let m1 = match f1.metadata() {
                Ok(m) => m,
                Err(_) => return None,
            };
            let m2 = match f2.metadata() {
                Ok(m) => m,
                Err(_) => return None,
            };
            Some(m1.dev() == m2.dev() && m1.ino() == m2.ino())
        };
        cmp_meta(&self.file, &other.file).unwrap_or_else(|| self.path == other.path)
    }
}

impl TryFrom<&str> for NetNS {
    type Error = Error;
    fn try_from(path: &str) -> Result<Self, Self::Error> {
        let p = PathBuf::from(path);
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) => {
                if e.kind().eq(&std::io::ErrorKind::NotFound) {
                    return Err(Error::NotExist(p));
                }
                return Err(Error::OpenNetNS(p, e));
            }
        };
        Ok(NetNS { path: p, file })
    }
}

impl TryFrom<PathBuf> for NetNS {
    type Error = Error;
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let file =
            std::fs::File::open(path.clone()).map_err(|e| Error::OpenNetNS(path.clone(), e))?;
        Ok(NetNS { path, file })
    }
}

impl TryFrom<&PathBuf> for NetNS {
    type Error = Error;
    fn try_from(path: &PathBuf) -> Result<Self, Self::Error> {
        let file =
            std::fs::File::open(path.clone()).map_err(|e| Error::OpenNetNS(path.clone(), e))?;
        Ok(NetNS {
            path: path.clone(),
            file,
        })
    }
}

impl std::fmt::Display for NetNS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.path().as_os_str().to_str() {
            Some(p) => write!(f, "{}", p),
            None => write!(f, "invalid netns path"),
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Open NetNS {0} : {1}")]
    OpenNetNS(PathBuf, std::io::Error),

    #[error("NetNS doesn't exist: {0}")]
    NotExist(PathBuf),

    #[error("Close NetNS")]
    CloseNetNS,

    #[error("SetNS: {0}")]
    SetNS(#[source] nix::Error),

    #[error("Netlink: {0}")]
    Netlink(#[source] netlink::Error),
}

#[cfg(test)]
mod tests {

    use super::{get_current_netns, NetNS};

    #[tokio::test]
    async fn test_netns_enter() {
        let base_ns = get_current_netns().unwrap();
        let mut cmd = std::process::Command::new("ip");
        cmd.args(["netns", "add", "test"]);
        cmd.output().unwrap();

        let target = NetNS::try_from("/var/run/netns/test").unwrap();

        target.enter().unwrap();

        base_ns.enter().unwrap();

        let mut cmd = std::process::Command::new("ip");
        cmd.args(["netns", "del", "test"]);
        cmd.output().unwrap();
    }
}
