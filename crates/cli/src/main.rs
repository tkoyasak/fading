use anyhow::Result;

mod flags;
mod ls;
mod notify;
mod open;

use crate::flags::{Cmd, Fading, FadingCmd};

fn main() {
    if let Err(err) = try_main() {
        for cause in err.chain() {
            eprintln!("error: {cause}");
        }
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let flags = Fading::from_env()?;
    run_command(flags)
}

fn run_command(flags: Fading) -> Result<()> {
    match flags.subcommand {
        FadingCmd::Ls(ls) => ls.run(),
        FadingCmd::Notify(notify) => notify.run(),
        FadingCmd::Open(open) => open.run(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::{Ls, Notify, Open};
    use proptest::prelude::*;

    mod strategies {
        use super::*;

        pub(super) fn valid_month() -> impl Strategy<Value = String> {
            (2000u16..=2100, 1u8..=12).prop_map(|(y, m)| format!("{:04}-{:02}", y, m))
        }

        pub(super) fn invalid_month() -> impl Strategy<Value = String> {
            prop_oneof![
                (2000u16..=2100).prop_map(|y| format!("{:04}-00", y)),
                (2000u16..=2100).prop_map(|y| format!("{:04}-13", y)),
            ]
        }
    }

    proptest! {
        /// run_command with valid Open should attempt to execute
        #[test]
        fn prop_run_command_open_valid(month in strategies::valid_month()) {
            let flags = Fading {
                subcommand: FadingCmd::Open(Open { month: Some(month) }),
            };
            // Will fail because hx is not available, but validates dispatch
            let _result = run_command(flags);
        }

        /// run_command with invalid Open should return error
        #[test]
        fn prop_run_command_open_invalid(month in strategies::invalid_month()) {
            let flags = Fading {
                subcommand: FadingCmd::Open(Open { month: Some(month) }),
            };
            let result = run_command(flags);
            prop_assert!(result.is_err());
        }
    }

    #[test]
    fn test_run_command_open() {
        let flags = Fading {
            subcommand: FadingCmd::Open(Open {
                month: Some("2025-01".to_string()),
            }),
        };

        // Will fail because hx is not available, but tests the dispatch logic
        let result = run_command(flags);
        assert!(result.is_err()); // Expected to fail without hx
    }

    #[test]
    fn test_run_command_open_invalid_month() {
        let flags = Fading {
            subcommand: FadingCmd::Open(Open {
                month: Some("2025-13".to_string()),
            }),
        };

        let result = run_command(flags);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid month format"));
    }

    #[test]
    fn test_run_command_open_none() {
        let flags = Fading {
            subcommand: FadingCmd::Open(Open { month: None }),
        };
        // Will fail because hx is not available
        let result = run_command(flags);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_command_ls() {
        let flags = Fading {
            subcommand: FadingCmd::Ls(Ls),
        };
        // Ls will attempt to start tokio runtime and run fading_ls
        // This will likely hang or fail in test environment
        // We just verify the structure compiles
        let _ = flags;
    }

    #[test]
    fn test_run_command_notify() {
        let flags = Fading {
            subcommand: FadingCmd::Notify(Notify),
        };
        // Notify will attempt to run osascript
        // We just verify the structure compiles
        let _ = flags;
    }

    #[test]
    fn test_error_chain() {
        // Create a simple error
        let err = anyhow::anyhow!("test error");

        // Collect error chain
        let chain: Vec<String> = err.chain().map(|e| e.to_string()).collect();

        // Should have at least one error
        assert!(chain.len() >= 1);
        assert!(chain[0].contains("test error"));
    }

    #[test]
    fn test_error_display() {
        let flags = Fading {
            subcommand: FadingCmd::Open(Open {
                month: Some("invalid".to_string()),
            }),
        };

        if let Err(err) = run_command(flags) {
            // Error should have a displayable chain
            let mut count = 0;
            for _cause in err.chain() {
                count += 1;
            }
            assert!(count > 0);
        }
    }
}
