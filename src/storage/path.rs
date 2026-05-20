use std::path::PathBuf;

/// `~/.ssh/config.d/sshc.conf`
pub fn sshc_conf_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config.d").join("sshc.conf"))
}

/// `~/.ssh/config.d`
pub fn ssh_config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config.d"))
}

/// `~/.ssh/config`
pub fn main_ssh_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ssh").join("config"))
}
