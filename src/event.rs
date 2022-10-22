//! Event-related utilities.

use std::{
    cell::RefCell,
    collections::VecDeque,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use futures::stream::FusedStream;
use wasm_bindgen::{convert::FromWasmAbi, prelude::Closure, JsCast};
use web_sys::EventTarget;

use crate::{closure, JsError};

/// Trait for listening to events with a callback.
pub trait When: AsRef<EventTarget> + Sized {
    /// Run `callback` when given event type occurs.
    fn when<E: FromWasmAbi + 'static, F: FnMut(E) + 'static>(
        self: &Rc<Self>,
        event_type: &'static str,
        callback: F,
    ) -> Result<EventListener<Self, E>, JsError>;
}

/// Trait for creating event streams.
pub trait Stream: When {
    /// Create stream of given event type.
    fn listen<E: FromWasmAbi + 'static>(
        self: &Rc<Self>,
        event_type: &'static str,
    ) -> Result<EventStream<Self, E>, JsError>;
}

/// Listener of events.
/// 
/// Drop to remove event listener.
#[derive(Debug)]
pub struct EventListener<T, E>
where
    T: AsRef<EventTarget>,
{
    event_type: &'static str,
    target: Rc<T>,
    closure: Closure<dyn FnMut(E)>,
}

impl<T, E> Drop for EventListener<T, E>
where
    T: AsRef<EventTarget>,
{
    fn drop(&mut self) {
        let _ = self
            .target
            .as_ref()
            .as_ref()
            .remove_event_listener_with_callback(
                self.event_type,
                self.closure.as_ref().unchecked_ref(),
            );
    }
}

impl<T> When for T
where
    T: AsRef<EventTarget>,
{
    fn when<E: FromWasmAbi + 'static, F: FnMut(E) + 'static>(
        self: &Rc<Self>,
        event_type: &'static str,
        callback: F,
    ) -> Result<EventListener<Self, E>, JsError> {
        let closure = closure!(callback);
        self.as_ref()
            .as_ref()
            .add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())?;
        Ok(EventListener {
            event_type,
            target: self.clone(),
            closure,
        })
    }
}

/// Stream of events.
#[derive(Debug)]
pub struct EventStream<T, E>
where
    T: When,
{
    state: Rc<RefCell<State<E>>>,
    listener: Option<EventListener<T, E>>,
}

impl<T, E> EventStream<T, E>
where
    T: AsRef<EventTarget>,
{
    /// Stop listening to events.
    /// 
    /// This means stream will terminate as soon as all received before events are consumed.
    pub fn stop(&mut self) {
        self.listener = None;
        if let Some(waker) = &self.state.borrow().waker {
            waker.wake_by_ref();
        }
    }
}

#[derive(Debug)]
struct State<E> {
    queue: VecDeque<E>,
    waker: Option<Waker>,
}

impl<T, E> Unpin for EventStream<T, E> where T: AsRef<EventTarget> {}

impl<T, E> futures::Stream for EventStream<T, E>
where
    T: AsRef<EventTarget>,
{
    type Item = E;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut state = self.state.borrow_mut();
        if let Some(event) = state.queue.pop_front() {
            Poll::Ready(Some(event))
        } else {
            if self.listener.is_none() {
                Poll::Ready(None)
            } else {
                let new_waker = cx.waker();
                if let Some(waker) = &mut state.waker {
                    if !waker.will_wake(new_waker) {
                        state.waker = Some(new_waker.clone());
                    }
                } else {
                    state.waker = Some(new_waker.clone());
                }
                Poll::Pending
            }
        }
    }
}

impl<T, E> FusedStream for EventStream<T, E>
where
    T: AsRef<EventTarget>,
{
    fn is_terminated(&self) -> bool {
        self.listener.is_none() && self.state.borrow().queue.is_empty()
    }
}

impl<T> Stream for T
where
    T: When,
{
    fn listen<E: FromWasmAbi + 'static>(
        self: &Rc<Self>,
        event_type: &'static str,
    ) -> Result<EventStream<Self, E>, JsError> {
        let state = Rc::new(RefCell::new(State {
            queue: VecDeque::new(),
            waker: None,
        }));
        let state_clone = state.clone();
        let listener = self.when(event_type, move |event| {
            let mut state = state_clone.borrow_mut();
            state.queue.push_back(event);
            if let Some(waker) = &state.waker {
                waker.wake_by_ref();
            }
        })?;
        let event_stream = EventStream {
            state,
            listener: Some(listener),
        };
        Ok(event_stream)
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc, time::Duration};

    use futures::StreamExt;
    use wasm_bindgen_test::wasm_bindgen_test;
    use web_sys::MouseEvent;

    use crate::{
        body,
        event::{EventStream, Stream, When},
        sleep, spawn,
    };

    #[wasm_bindgen_test]
    async fn test_event_listener() {
        let body = Rc::new(body());

        let result = Rc::new(Cell::new(None));
        let result_clone = result.clone();
        let _listener = body
            .when("click", move |_: MouseEvent| {
                result_clone.set(Some("Done!"));
            })
            .unwrap();
        body.click();
        sleep(Duration::from_secs_f32(1.1)).await;

        assert_eq!(result.take().unwrap(), "Done!");
    }

    #[wasm_bindgen_test]
    async fn test_event_stream() {
        let body = Rc::new(body());

        let body_clone = body.clone();
        let handle = spawn(async move {
            let mut stream: EventStream<_, MouseEvent> = body_clone.listen("click").unwrap();
            stream.next().await.unwrap();
            stream.next().await.unwrap();
            stream.stop();
        });
        sleep(Duration::from_secs_f32(0.1)).await;
        body.click();
        body.click();
        let _ = handle.await;

        let mut stream: EventStream<_, MouseEvent> = body.listen("click").unwrap();
        body.click();
        body.click();
        body.click();
        stream.stop();
        body.click();
        body.click();

        let mut c = 0;
        assert_eq!(
            stream
                .map(move |_: MouseEvent| {
                    c += 1;
                    c
                })
                .collect::<Vec<i32>>()
                .await,
            vec![1, 2, 3]
        );
    }
}
