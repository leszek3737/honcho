use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::{Handle, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_or_create_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to create tokio runtime: {e}");
            std::process::exit(1);
        }
    })
}

#[expect(clippy::panic)]
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    match Handle::try_current() {
        Ok(_) => panic!(
            "blocking::Honcho cannot be called from within an async runtime. \
             Use the async Honcho client instead."
        ),
        Err(_) => get_or_create_runtime().block_on(future),
    }
}
