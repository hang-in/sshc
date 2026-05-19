#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Connects to the given SSH host by replacing the current process.
///
/// # Safety
/// The caller MUST ensure terminal state is fully restored before calling this function.
/// After `exec()`, the current process is replaced — no code after this call will run.
#[cfg(unix)]
pub fn ssh_connect(alias: &str) -> anyhow::Result<()> {
    std::io::stdout().flush()?;
    let err = Command::new("ssh").arg(alias).exec();
    // exec() only returns on error
    Err(anyhow::anyhow!("ssh exec failed: {}", err))
}

#[cfg(not(unix))]
pub fn ssh_connect(_alias: &str) -> anyhow::Result<()> {
    Err(anyhow::anyhow!("sshs only supports Unix systems"))
}

use std::io::Write;
