//! Async queue.

use futures::{future::FusedFuture, Future};
use std::{
    cell::RefCell,
    collections::VecDeque,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context, Poll, Waker},
};

/// FIFO queue with async pop.
pub struct Queue<T> {
    state: RefCell<State<T>>,
    capacity: usize,
}

struct State<T> {
    buffer: VecDeque<T>,
    wakers: VecDeque<Weak<RefCell<PopWaker>>>,
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
            state: RefCell::new(State::new()),
            capacity: 0,
        }
    }

    /// Creates new queue with given `capacity`.
    ///
    /// `capacity` must be greater than 0 - it'll panic otherwise.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        Queue {
            state: RefCell::new(State::new()),
            capacity,
        }
    }

    /// Pushes `element` into the queue.
    ///
    /// If queue is full it will push out the last (oldest) element
    /// out of the queue.
    pub fn push(&self, element: T) {
        let mut state = self.state.borrow_mut();
        state.buffer.push_front(element);
        if self.capacity > 0 {
            state.buffer.truncate(self.capacity)
        }
        drop(state);
        self.wake_next();
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
            waker: None,
        }
    }

    /// Pops element off the queue.
    ///
    /// Returns `None` if queue is currently empty.
    pub fn try_pop(&self) -> Option<T> {
        self.state.borrow_mut().buffer.pop_back()
    }

    /// Returns count of elements currently in the queue.
    pub fn len(&self) -> usize {
        self.state.borrow_mut().buffer.len()
    }

    /// Returns `true` if queue is currently empty.
    pub fn is_empty(&self) -> bool {
        self.state.borrow_mut().buffer.is_empty()
    }

    /// Returns `true` if queue is currently full.
    pub fn is_full(&self) -> bool {
        if self.capacity == 0 {
            false
        } else {
            self.len() == self.capacity
        }
    }

    fn wake_next(&self) {
        while let Some(waker) = self.state.borrow_mut().wakers.pop_front() {
            if let Some(waker) = waker.upgrade() {
                let mut waker = waker.borrow_mut();
                waker.woken = true;
                waker.waker.wake_by_ref();
                break;
            }
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
    waker: Option<Rc<RefCell<PopWaker>>>,
}

struct PopWaker {
    waker: Waker,
    woken: bool,
}

impl PopWaker {
    fn new(waker: Waker) -> Self {
        PopWaker {
            waker,
            woken: false,
        }
    }

    fn update(&mut self, waker: &Waker) {
        if !self.waker.will_wake(waker) {
            self.waker = waker.clone();
        }
    }
}

impl<'a, T> Drop for Pop<'a, T> {
    fn drop(&mut self) {
        // We were woken but didn't receive anything, wake up another
        if self
            .waker
            .take()
            .map_or(false, |waker| waker.borrow().woken)
        {
            self.queue.wake_next();
        }
    }
}

impl<'a, T> Future for Pop<'a, T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.terminated {
            Poll::Pending
        } else {
            let mut state = self.queue.state.borrow_mut();
            match state.buffer.pop_back() {
                Some(value) => {
                    self.terminated = true;
                    self.waker = None;
                    Poll::Ready(value)
                }
                None => {
                    if let Some(waker) = &self.waker {
                        let mut waker = waker.borrow_mut();
                        waker.update(cx.waker());
                        waker.woken = false;
                    } else {
                        let waker = Rc::new(RefCell::new(PopWaker::new(cx.waker().clone())));
                        self.waker = Some(waker);
                    }
                    state
                        .wakers
                        .push_front(Rc::downgrade(self.waker.as_ref().unwrap()));
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

    use futures::{join, FutureExt};
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

        assert_eq!(queue.pop().now_or_never(), None);
        assert_eq!(queue.pop().now_or_never(), None);
        assert_eq!(queue.pop().now_or_never(), None);
        let queue_clone = queue.clone();
        let task = spawn(async move {
            assert_eq!(queue_clone.pop().now_or_never(), None);
            assert_eq!(queue_clone.pop().now_or_never(), None);
            join![queue_clone.pop(), queue_clone.pop(), queue_clone.pop()]
        });
        sleep(Duration::from_secs(1)).await;
        queue.push(1);
        queue.push(2);
        queue.push(3);

        assert_eq!(task.await.unwrap(), (1, 2, 3));
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
