use std::env;
use std::path::Path;
use std::process::Command;

/// Builds the editor command for opening a config file at a specific line.
///
/// For vim-like editors (vi, vim, nvim, nano), the `+<line>` flag is added
/// to jump to the specified line. For other editors, the file is opened
/// without a line specifier.
pub fn build_editor_command(file: &Path, line: usize) -> Command {
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let mut cmd = Command::new(&editor);

    if is_vim_like(&editor) && line > 0 {
        cmd.arg(format!("+{}", line));
    }

    cmd.arg(file);
    cmd
}

/// Returns true if the editor supports the `+<line>` flag for jumping to a line.
fn is_vim_like(editor: &str) -> bool {
    let base = Path::new(editor)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    matches!(base, "vi" | "vim" | "nvim" | "nano" | "nano-tiny")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_editor_command_construction() {
        // Test with vim-like editor
        let file = PathBuf::from("/test/config");
        let cmd = build_editor_command(&file, 42);

        // Should include +42 flag for vim
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&"+42".to_string()));
        assert!(args.iter().any(|a| a.contains("config")));
    }

    #[test]
    fn test_editor_fallback_to_vi() {
        // Without EDITOR set, should default to "vi"
        // Note: this test assumes EDITOR is not set to something unusual
        let original = env::var("EDITOR").ok();
        env::remove_var("EDITOR");

        let file = PathBuf::from("/test/config");
        let cmd = build_editor_command(&file, 1);

        let program = cmd.get_program().to_string_lossy().to_string();
        assert_eq!(program, "vi");

        // Restore EDITOR
        if let Some(val) = original {
            env::set_var("EDITOR", val);
        }
    }

    #[test]
    fn test_is_vim_like() {
        assert!(is_vim_like("vi"));
        assert!(is_vim_like("vim"));
        assert!(is_vim_like("nvim"));
        assert!(is_vim_like("nano"));
        assert!(!is_vim_like("code"));
        assert!(!is_vim_like("emacs"));
        assert!(!is_vim_like("subl"));
        assert!(!is_vim_like("/usr/bin/code"));
    }
}
