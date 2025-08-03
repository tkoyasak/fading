use zed_extension_api as zed;

struct FadingExtension;

impl zed::Extension for FadingExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        match worktree.which("fading-ls") {
            Some(path) => Ok(zed::Command::new(path)),
            None => Err("`fading-ls` bin not found.".to_string()),
        }
    }
}

zed::register_extension!(FadingExtension);
