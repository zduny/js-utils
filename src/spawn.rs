//! Background task spawning.

use futures::Future;
use std::rc::Rc;
use std::sync::Mutex;
use std::task::{Poll, Waker};

/// Spawns a new asynchronous task, returning a [`JoinHandle`] for it.
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + 'static,
    F::Output: 'static,
{
    let join_handle = JoinHandle::new();
    let join_handle_clone = join_handle.clone();
    wasm_bindgen_futures::spawn_local(async move {
        join_handle_clone.set_result(future.await);
    });
    join_handle
}

/// Task failed to execute to completion.
///
/// Currently can only be caused by cancellation.
#[derive(Debug)]
#[non_exhaustive]
pub struct JoinError {}

impl JoinError {
    /// Returns true if the error was caused by the task being cancelled.
    pub fn is_cancelled(&self) -> bool {
        true
    }
}

/// An owned permission to join on a task (await its termination).
///
/// This can be thought of as the equivalent of [`std::thread::JoinHandle`] for
/// a task rather than a thread.
///
/// A `JoinHandle` *detaches* the associated task when it is dropped, which
/// means that there is no longer any handle to the task, and no way to `join`
/// on it.
///
/// This `struct` is created by the [`spawn`] function.
#[derive(Debug)]
pub struct JoinHandle<T> {
    state: Rc<Mutex<State<T>>>,
}

impl<T> JoinHandle<T> {
    fn new() -> Self {
        JoinHandle {
            state: Rc::new(Mutex::new(State::new())),
        }
    }

    /// Abort the task associated with the handle.
    ///
    /// Awaiting a cancelled task might complete as usual if the task was
    /// already completed at the time it was cancelled, but most likely it
    /// will fail with a [cancelled] [`JoinError`].
    ///
    /// [cancelled]: method@crate::spawn::JoinError::is_cancelled
    pub fn abort(&self) {
        self.state.lock().unwrap().set_result(Err(JoinError {}));
    }

    /// Checks if the task associated with this `JoinHandle` has finished.
    ///
    /// Please note that this method can return `false` even if [`abort`] has been
    /// called on the task. This is because the cancellation process may take
    /// some time, and this method does not return `true` until it has
    /// completed.
    ///
    /// [`abort`]: method@JoinHandle::abort
    pub fn is_finished(&self) -> bool {
        self.state.lock().unwrap().is_finished()
    }

    fn set_result(&self, value: T) {
        self.state.lock().unwrap().set_result(Ok(value));
    }

    fn clone(&self) -> Self {
        JoinHandle {
            state: self.state.clone(),
        }
    }
}

#[derive(Debug)]
struct State<T> {
    result: Option<Result<T, JoinError>>,
    waker: Option<Waker>,
}

impl<T> State<T> {
    fn new() -> Self {
        State {
            result: None,
            waker: None,
        }
    }

    fn is_finished(&self) -> bool {
        self.result.is_some()
    }

    fn set_result(&mut self, value: Result<T, JoinError>) {
        if self.result.is_none() {
            self.result = Some(value);
            self.wake();
        }
    }

    fn wake(&mut self) {
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    fn update_waker(&mut self, waker: &Waker) {
        if let Some(current_waker) = &self.waker {
            if !waker.will_wake(current_waker) {
                self.waker = Some(waker.clone());
            }
        } else {
            self.waker = Some(waker.clone())
        }
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();
        if let Some(value) = state.result.take() {
            Poll::Ready(value)
        } else {
            state.update_waker(cx.waker());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{sleep, spawn};

    #[wasm_bindgen_test]
    async fn test_spawn() {
        let task_1 = spawn(async { 1 });
        let task_2 = spawn(async { 2 });

        sleep(Duration::from_secs(1)).await;

        assert!(task_1.is_finished());
        assert!(task_2.is_finished());

        assert_eq!(task_1.await.unwrap(), 1);
        assert_eq!(task_2.await.unwrap(), 2);
    }

    #[wasm_bindgen_test]
    async fn test_abort() {
        let task = spawn(async {
            sleep(Duration::from_secs(10)).await;
            1
        });
        task.abort();

        assert!(task.await.unwrap_err().is_cancelled());
    }
}
