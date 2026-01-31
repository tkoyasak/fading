use tokio::runtime::Runtime;

use crate::flags::{Cmd, Ls};

impl Cmd for Ls {
    fn run(self) -> anyhow::Result<()> {
        let rt = Runtime::new()?;
        rt.block_on(fading_ls::run());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn test_ls_struct_exists() {
        let _ls = Ls;
        // Ls is a unit struct, just verify it exists
    }

    #[test]
    fn test_runtime_creation() {
        // Verify we can create a runtime
        let result = Runtime::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_runtime_block_on() {
        let rt = Runtime::new().unwrap();
        let result = rt.block_on(async { 42 });
        assert_eq!(result, 42);
    }

    #[test]
    fn test_async_execution() {
        let rt = Runtime::new().unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        rt.block_on(async move {
            flag_clone.store(true, Ordering::SeqCst);
        });

        assert!(flag.load(Ordering::SeqCst));
    }
}
