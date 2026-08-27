use zed_extension_api as zed;

struct RosyExtension;

impl zed::Extension for RosyExtension {
    fn new() -> Self {
        RosyExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let path = worktree
            .which("rosy")
            .ok_or_else(|| {
                "rosy not found in PATH — install with: cargo install rosy-cli".to_string()
            })?;

        Ok(zed::Command {
            command: path,
            args: vec!["lsp".to_string()],
            env: vec![],
        })
    }
}

zed::register_extension!(RosyExtension);
