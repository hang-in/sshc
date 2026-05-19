use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum SshError {
    LaunchFailed(std::io::Error),
    WaitFailed(std::io::Error),
}

impl fmt::Display for SshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SshError::LaunchFailed(e) => write!(f, "failed to launch ssh: {e}"),
            SshError::WaitFailed(e) => write!(f, "failed to wait for ssh: {e}"),
        }
    }
}

impl Error for SshError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SshError::LaunchFailed(e) => Some(e),
            SshError::WaitFailed(e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub enum TerminalError {
    EnterRawMode(std::io::Error),
    EnterAltScreen(std::io::Error),
    LeaveAltScreen(std::io::Error),
    LeaveRawMode(std::io::Error),
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalError::EnterRawMode(e) => write!(f, "failed to enable raw mode: {e}"),
            TerminalError::EnterAltScreen(e) => write!(f, "failed to enter alt screen: {e}"),
            TerminalError::LeaveAltScreen(e) => write!(f, "failed to leave alt screen: {e}"),
            TerminalError::LeaveRawMode(e) => write!(f, "failed to disable raw mode: {e}"),
        }
    }
}

impl Error for TerminalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TerminalError::EnterRawMode(e) => Some(e),
            TerminalError::EnterAltScreen(e) => Some(e),
            TerminalError::LeaveAltScreen(e) => Some(e),
            TerminalError::LeaveRawMode(e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub enum EditorError {
    LaunchFailed(std::io::Error),
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorError::LaunchFailed(e) => write!(f, "failed to launch editor: {e}"),
        }
    }
}

impl Error for EditorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EditorError::LaunchFailed(e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub enum StorageError {
    LockFailed(std::io::Error),
    LockHeldByOther,
    ReadFailed(std::io::Error),
    WriteFailed(std::io::Error),
    RenameFailed(std::io::Error),
    BackupFailed(std::io::Error),
    PermissionMismatch {
        path: std::path::PathBuf,
        expected: u32,
        actual: u32,
    },
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::LockFailed(e) => write!(f, "failed to acquire lock: {e}"),
            StorageError::LockHeldByOther => write!(f, "sshs.conf locked by another instance"),
            StorageError::ReadFailed(e) => write!(f, "failed to read: {e}"),
            StorageError::WriteFailed(e) => write!(f, "failed to write: {e}"),
            StorageError::RenameFailed(e) => write!(f, "failed to commit write (rename): {e}"),
            StorageError::BackupFailed(e) => write!(f, "failed to create backup: {e}"),
            StorageError::PermissionMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "permission mismatch on {path:?}: expected {expected:o}, found {actual:o}"
            ),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            StorageError::LockFailed(e) => Some(e),
            StorageError::ReadFailed(e) => Some(e),
            StorageError::WriteFailed(e) => Some(e),
            StorageError::RenameFailed(e) => Some(e),
            StorageError::BackupFailed(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum SetupError {
    HomeDirMissing,
    MkdirFailed(std::io::Error),
    Storage(StorageError),
    StateParseFailed(String),
    StateWriteFailed(StorageError),
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::HomeDirMissing => write!(f, "could not locate home directory"),
            SetupError::MkdirFailed(e) => write!(f, "failed to create directory: {e}"),
            SetupError::Storage(e) => write!(f, "{e}"),
            SetupError::StateParseFailed(s) => write!(f, "failed to parse state.toml: {s}"),
            SetupError::StateWriteFailed(e) => write!(f, "failed to save state.toml: {e}"),
        }
    }
}

impl Error for SetupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SetupError::MkdirFailed(e) => Some(e),
            SetupError::Storage(e) => Some(e),
            SetupError::StateWriteFailed(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ProbeError {
    ResolveFailed(std::io::Error),
    ConnectTimeout,
    ConnectRefused(std::io::Error),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::ResolveFailed(e) => write!(f, "failed to resolve host: {e}"),
            ProbeError::ConnectTimeout => write!(f, "connect timed out"),
            ProbeError::ConnectRefused(e) => write!(f, "connection refused: {e}"),
        }
    }
}

impl Error for ProbeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProbeError::ResolveFailed(e) => Some(e),
            ProbeError::ConnectRefused(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum AppError {
    Terminal(TerminalError),
    Ssh(SshError),
    Editor(EditorError),
    Io(std::io::Error),
    Storage(StorageError),
    Setup(SetupError),
    Probe(ProbeError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Terminal(e) => write!(f, "{e}"),
            AppError::Ssh(e) => write!(f, "{e}"),
            AppError::Editor(e) => write!(f, "{e}"),
            AppError::Io(e) => write!(f, "{e}"),
            AppError::Storage(e) => write!(f, "{e}"),
            AppError::Setup(e) => write!(f, "{e}"),
            AppError::Probe(e) => write!(f, "{e}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AppError::Terminal(e) => Some(e),
            AppError::Ssh(e) => Some(e),
            AppError::Editor(e) => Some(e),
            AppError::Io(e) => Some(e),
            AppError::Storage(e) => Some(e),
            AppError::Setup(e) => Some(e),
            AppError::Probe(e) => Some(e),
        }
    }
}

impl From<TerminalError> for AppError {
    fn from(err: TerminalError) -> Self {
        AppError::Terminal(err)
    }
}

impl From<SshError> for AppError {
    fn from(err: SshError) -> Self {
        AppError::Ssh(err)
    }
}

impl From<EditorError> for AppError {
    fn from(err: EditorError) -> Self {
        AppError::Editor(err)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<StorageError> for AppError {
    fn from(err: StorageError) -> Self {
        AppError::Storage(err)
    }
}

impl From<SetupError> for AppError {
    fn from(err: SetupError) -> Self {
        AppError::Setup(err)
    }
}

impl From<ProbeError> for AppError {
    fn from(err: ProbeError) -> Self {
        AppError::Probe(err)
    }
}
