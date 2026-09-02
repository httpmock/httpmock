use std::sync::Mutex;

use tokio::sync::Semaphore;

/// A simple async object pool holding up to `capacity` objects.
///
/// [`Pool::take_or_create`] returns a pooled object, or creates a new one if the pool has
/// spare capacity. When `capacity` objects are already handed out, it waits asynchronously
/// until one is returned via [`Pool::put`].
///
/// This implementation must stay runtime agnostic, because it is used on the client side
/// where user code may run on any async runtime (e.g., smol or actix-rt, see
/// `tests/misc/runtimes_test.rs`). It therefore only uses `tokio::sync` primitives, which
/// are runtime agnostic (they do not require the tokio reactor or timer, unlike
/// `tokio::time`, `tokio::net`, etc.).
pub struct Pool<T> {
    items: Mutex<Vec<T>>,
    // Each permit represents the right to hold one of the pool's objects. Waiting for a
    // permit is what suspends callers while all objects are handed out.
    permits: Semaphore,
}

impl<T> Pool<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            permits: Semaphore::new(capacity.min(Semaphore::MAX_PERMITS)),
        }
    }

    pub async fn take_or_create<F>(&self, create: F) -> T
    where
        F: Fn() -> T,
    {
        let permit = self.permits.acquire().await.expect("object pool semaphore was closed");

        let existing = self.items.lock().expect("object pool lock poisoned").pop();
        // `create` may block for a while (e.g., starting a mock server), so call it
        // outside of the lock. If it panics, the still-attached permit is released on
        // unwind, so the pool's capacity is not reduced by failed creation attempts.
        let item = existing.unwrap_or_else(create);

        // The permit is restored by `put` when the object is returned to the pool.
        permit.forget();
        item
    }

    pub fn put(&self, item: T) {
        self.items.lock().expect("object pool lock poisoned").push(item);
        self.permits.add_permits(1);
    }
}

#[cfg(test)]
mod test {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::Pool;

    #[tokio::test]
    async fn reuses_returned_objects() {
        let created = AtomicUsize::new(0);
        let pool = Pool::new(5);

        let first = pool.take_or_create(|| created.fetch_add(1, Ordering::SeqCst)).await;
        pool.put(first);
        let second = pool.take_or_create(|| created.fetch_add(1, Ordering::SeqCst)).await;

        assert_eq!(first, second);
        assert_eq!(created.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn waits_for_returned_object_when_capacity_reached() {
        let pool = Arc::new(Pool::new(1));

        let taken = pool.take_or_create(|| 1).await;

        let waiter = {
            let pool = pool.clone();
            tokio::spawn(async move { pool.take_or_create(|| unreachable!("pool is at capacity")).await })
        };

        // Let the spawned task run until it suspends waiting for a permit.
        tokio::task::yield_now().await;

        // The waiter cannot obtain an object until the first one is returned.
        assert!(!waiter.is_finished());
        pool.put(taken);
        assert_eq!(waiter.await.unwrap(), 1);
    }

    #[tokio::test]
    async fn panicking_creator_does_not_reduce_capacity() {
        let pool = Arc::new(Pool::new(1));

        let panicking = {
            let pool = pool.clone();
            tokio::spawn(async move { pool.take_or_create(|| panic!("creation failed")).await })
        };
        assert!(panicking.await.is_err());

        // The failed creation attempt must not consume the pool's only permit.
        assert_eq!(pool.take_or_create(|| 42).await, 42);
    }

    // The pool must not depend on the tokio runtime (see module documentation).
    #[test]
    fn works_without_tokio_runtime() {
        smol::block_on(async {
            let pool = Pool::new(1);
            let value = pool.take_or_create(|| 42).await;
            pool.put(value);
            assert_eq!(pool.take_or_create(|| 0).await, 42);
        });
    }
}
