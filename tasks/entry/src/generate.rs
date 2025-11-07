use crate::{
    entry::generate_entry,
    flags::{Cmd, Generate},
    github::create_pull_request,
};

impl Cmd for Generate {
    fn run(self) -> anyhow::Result<()> {
        let entry = generate_entry()?;
        create_pull_request(entry)
    }
}
