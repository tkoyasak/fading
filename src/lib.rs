use zed_extension_api as zed;

struct FadingExtension;

impl zed::Extension for FadingExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        Ok(zed::Command {
            command: env!("LS_PATH").to_owned(),
            args: Default::default(),
            env: Default::default(),
        })
    }
}

zed::register_extension!(FadingExtension);
