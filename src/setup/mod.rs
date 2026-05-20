pub mod detect;
pub mod permissions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupOutcome {
    Ready,
    AwaitingIncludeChoice,
    ReadOnly,
}

/// Run first-run / startup checks. Ensures the sshc.conf scaffolding is in
/// place, then determines whether the main ssh_config already includes it
/// (Ready), whether the user must be prompted (AwaitingIncludeChoice), or
/// whether the user previously declined (ReadOnly).
pub fn run_first_run_checks(
    state: &mut crate::state::State,
) -> Result<SetupOutcome, crate::error::SetupError> {
    use crate::error::SetupError;

    let sshc_conf = crate::storage::sshc_conf_path().ok_or(SetupError::HomeDirMissing)?;
    let main_config = crate::storage::main_ssh_config_path().ok_or(SetupError::HomeDirMissing)?;
    let config_dir = crate::storage::ssh_config_dir().ok_or(SetupError::HomeDirMissing)?;

    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(SetupError::MkdirFailed)?;
    }
    permissions::ensure_dir_mode(&config_dir, 0o700)?;

    if !sshc_conf.exists() {
        crate::storage::with_locked_write(&sshc_conf, true, |_| {
            crate::storage::SSHC_CONF_BANNER.to_string()
        })
        .map_err(SetupError::Storage)?;
    }
    permissions::ensure_file_mode(&sshc_conf, 0o600)?;

    if !state.setup.include_check_done {
        if !main_config.exists() {
            state.setup.include_check_done = true;
            return Ok(SetupOutcome::ReadOnly);
        }

        if detect::include_is_present(&main_config, &sshc_conf)? {
            state.setup.include_check_done = true;
            return Ok(SetupOutcome::Ready);
        }

        if state.setup.declined_include_injection {
            state.setup.include_check_done = true;
            return Ok(SetupOutcome::ReadOnly);
        }

        return Ok(SetupOutcome::AwaitingIncludeChoice);
    }

    if !main_config.exists() {
        return Ok(SetupOutcome::ReadOnly);
    }
    if detect::include_is_present(&main_config, &sshc_conf)? {
        Ok(SetupOutcome::Ready)
    } else {
        Ok(SetupOutcome::ReadOnly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outcome_equality() {
        assert_ne!(SetupOutcome::Ready, SetupOutcome::ReadOnly);
        assert_eq!(SetupOutcome::Ready, SetupOutcome::Ready);
        assert_ne!(SetupOutcome::AwaitingIncludeChoice, SetupOutcome::Ready);
    }
}
