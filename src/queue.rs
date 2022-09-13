//! Async queue.

use futures::{future::FusedFuture, Future};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::Mutex,
    task::{Context, Poll, Waker},
};

/// FIFO queue with async pop.
pub struct Queue<T> {
    state: Mutex<State<T>>,
    capacity: usize,
}

struct State<T> {
    buffer: VecDeque<T>,
    wakers: VecDeque<Waker>,
}

impl<T> State<T> {
    fn new() -> Self {
        State {
            buffer: VecDeque::new(),
            wakers: VecDeque::new(),
        }
    }
}

impl<T> Queue<T> {
    /// Creates new queue with unbounded capacity.
    pub fn new() -> Self {
        Queue {
            state: Mutex::new(State::new()),
            capacity: 0,
        }
    }

    /// Creates new queue with given `capacity`.
    ///
    /// `capacity` must be greater than 0 - it'll panic otherwise.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        Queue {
            state: Mutex::new(State::new()),
            capacity,
        }
    }

    /// Pushes `element` into the queue.
    ///
    /// If queue is full it will push out the last (oldest) element
    /// out of the queue.
    pub fn push(&self, element: T) {
        let state = &mut self.state.lock().unwrap();
        state.buffer.push_front(element);
        if self.capacity > 0 {
            state.buffer.truncate(self.capacity)
        }
        if let Some(waker) = state.wakers.pop_back() {
            waker.wake();
        }
    }

    /// Pops (asynchronously) element off the queue.
    ///
    /// It means that if queue is currently empty `await` will
    /// wait till element is pushed into the queue.
    #[must_use]
    pub fn pop(&self) -> Pop<T> {
        Pop {
            queue: self,
            terminated: false,
        }
    }

    /// Pops element off the queue.
    ///
    /// Returns `None` if queue is currently empty.
    pub fn try_pop(&self) -> Option<T> {
        self.state.lock().unwrap().buffer.pop_back()
    }

    /// Returns count of elements currently in the queue.
    pub fn len(&self) -> usize {
        self.state.lock().unwrap().buffer.len()
    }

    /// Returns `true` if queue is currently empty.
    pub fn is_empty(&self) -> bool {
        self.state.lock().unwrap().buffer.is_empty()
    }

    /// Returns `true` if queue is currently full.
    pub fn is_full(&self) -> bool {
        if self.capacity == 0 {
            false
        } else {
            self.len() == self.capacity
        }
    }
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Future returned by [pop] method.
///
/// [pop]: Queue::pop
pub struct Pop<'a, T> {
    queue: &'a Queue<T>,
    terminated: bool,
}

impl<'a, T> Future for Pop<'a, T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.terminated {
            Poll::Pending
        } else {
            let mut state = self.queue.state.lock().unwrap();
            match state.buffer.pop_back() {
                Some(value) => {
                    self.terminated = true;
                    Poll::Ready(value)
                }
                None => {
                    state.wakers.push_front(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
    }
}

impl<'a, T> FusedFuture for Pop<'a, T> {
    fn is_terminated(&self) -> bool {
        self.terminated
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{sleep, spawn, Queue};

    #[wasm_bindgen_test]
    async fn test_unbounded() {
        let queue = Queue::new();

        assert_eq!(queue.try_pop(), None);

        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
        assert!(!queue.is_full());

        queue.push(1);
        queue.push(2);
        queue.push(3);

        assert_eq!(queue.len(), 3);
        assert!(!queue.is_full());

        assert_eq!(queue.try_pop().unwrap(), 1);
        assert_eq!(queue.pop().await, 2);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pop().await, 3);

        assert_eq!(queue.len(), 0);
        assert!((queue.is_empty()));

        let queue = Rc::new(queue);
        let queue_clone = queue.clone();
        spawn(async move {
            sleep(Duration::from_secs(1)).await;
            queue_clone.push(4);
            queue_clone.push(5);
            sleep(Duration::from_secs(1)).await;
            queue_clone.push(6);
        });

        assert_eq!(queue.pop().await, 4);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pop().await, 5);
        assert_eq!(queue.pop().await, 6);

        assert_eq!(queue.len(), 0);
        assert!((queue.is_empty()));

        queue.push(1);
        queue.push(2);
        queue.push(3);
    }

    #[wasm_bindgen_test]
    async fn test_bounded() {
        let queue = Queue::with_capacity(3);

        assert_eq!(queue.try_pop(), None);

        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
        assert!(!queue.is_full());

        queue.push(1);
        queue.push(2);
        queue.push(3);

        assert_eq!(queue.len(), 3);
        assert!(queue.is_full());
    }
}
