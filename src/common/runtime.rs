use std::{future::Future, time::Duration};

use tokio::{runtime::Runtime, task::LocalSet};

pub(crate) async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await
}

pub(crate) fn block_on_current_thread<F, O>(f: F) -> O
where
    F: Future<Output = O>,
{
    let mut runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Cannot build local tokio runtime");

    LocalSet::new().block_on(&mut runtime, f)
}

pub(crate) fn new(worker_threads: usize, blocking_threads: usize) -> std::io::Result<Runtime> {
    assert!(
        worker_threads > 0,
        "Parameter worker_threads must be larger than 0"
    );
    assert!(
        blocking_threads > 0,
        "Parameter blocking_threads must be larger than 0"
    );

    return tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .max_blocking_threads(blocking_threads) // This is a maximum
        .enable_all()
        .build();
}
