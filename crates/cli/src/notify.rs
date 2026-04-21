use anyhow::Result;
use mac_notification_sys::{
    MainButton, Notification, NotificationResponse, get_bundle_identifier_or_default,
    set_application,
};
use xshell::Shell;

use crate::{Cmd, Notify, Open};

impl Cmd for Notify {
    fn run(self, sh: Shell) -> Result<()> {
        let bundle = get_bundle_identifier_or_default("fading");
        set_application(&bundle)?;

        let response = Notification::new()
            .title("fading")
            .message("Time to be fading!")
            .main_button(MainButton::SingleAction("Open"))
            .send()?;

        if let NotificationResponse::ActionButton(_) = response {
            Open { month: None }.run(sh)?;
        }

        Ok(())
    }
}
