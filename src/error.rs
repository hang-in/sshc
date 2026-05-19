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
pub enum AppError {
    Terminal(TerminalError),
    Ssh(SshError),
    Editor(EditorError),
    Io(std::io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Terminal(e) => write!(f, "{e}"),
            AppError::Ssh(e) => write!(f, "{e}"),
            AppError::Editor(e) => write!(f, "{e}"),
            AppError::Io(e) => write!(f, "{e}"),
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
