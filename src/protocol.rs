use std::error::Error;
use std::time::Duration;

use anyhow::Result;
use futures::stream;
use futures::{StreamExt, TryStreamExt};
use futures_util::{stream::Stream, Sink, SinkExt};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tungstenite::{Error as WsError, Message};

use crate::simulation::Simulation;

const FORWARD_HOURS: f64 = 1000.0;
const STEP_DELAY_MILLIS: u64 = 200;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", content = "params", rename_all = "lowercase")]
pub enum Event {
  Start {
    landscape: Vec<f64>,
    hours: f64,
  },
  Step,
  Progress {
    running: bool,
    time: f64,
    levels: Vec<f64>,
  },
  Pause,
  Resume,
  Forward,
  ForwardStep,
}

pub struct Protocol {
  simulation: Simulation,
}

impl Protocol {
  pub fn new() -> Self {
    Self {
      simulation: Simulation::new(),
    }
  }

  pub async fn run<'a, MessagesOut, MessagesIn, MessagesErr, FeedbackTx, FeedbackRx, FeedbackErr>(
    &mut self,
    outgoing_messages: MessagesOut,
    incoming_messages: MessagesIn,
    mut outgoing_feedback_loop: FeedbackTx,
    incoming_feedback_loop: FeedbackRx,
  ) -> Result<()>
  where
    MessagesOut: Sink<Message, Error = MessagesErr> + Unpin + Send + 'a,
    MessagesIn: Stream<Item = Result<Message, WsError>> + Unpin + Send + 'a,
    MessagesErr: Error + Send + Sync + 'static,
    FeedbackTx: Sink<Event, Error = FeedbackErr> + Clone + Unpin + Send + 'static,
    FeedbackRx: Stream<Item = Event> + Unpin + Send + 'a,
    FeedbackErr: Error + Send + Sync + 'static,
  {
    let mut outgoing_events = outgoing_messages.with_flat_map(message_from_event);

    let incoming_events = incoming_messages
      .map_err(anyhow::Error::from)
      .filter_map(event_from_try_message);

    let mut multiplexed_events = stream::select_all(vec![
      incoming_events.boxed(),
      incoming_feedback_loop.boxed(),
    ]);

    while let Some(event) = multiplexed_events.next().await {
      log::info!("Recv: {:?}", event);
      match event {
        Event::Start { landscape, hours } => {
          self.simulation.start(landscape.as_slice(), hours);
          send_progress(&self.simulation, &mut outgoing_events).await?;
          tokio::spawn(send_event_delayed(
            Event::Step,
            outgoing_feedback_loop.clone(),
            STEP_DELAY_MILLIS,
          ));
        }
        Event::Step if self.simulation.is_running() && !self.simulation.is_fast_forward() => {
          self.simulation.step();
          send_progress(&self.simulation, &mut outgoing_events).await?;
          if !self.simulation.is_finished() {
            tokio::spawn(send_event_delayed(
              Event::Step,
              outgoing_feedback_loop.clone(),
              STEP_DELAY_MILLIS,
            ));
          }
        }
        Event::Forward => {
          self.simulation.start_forward();
          send_progress(&self.simulation, &mut outgoing_events).await?;
          send_event(Event::ForwardStep, &mut outgoing_feedback_loop).await?;
        }
        Event::ForwardStep if self.simulation.is_running() && self.simulation.is_fast_forward() => {
          self.simulation.forward(FORWARD_HOURS);
          send_progress(&self.simulation, &mut outgoing_events).await?;
          if !self.simulation.is_finished() {
            send_event(Event::ForwardStep, &mut outgoing_feedback_loop).await?;
          }
        }
        Event::Pause => {
          self.simulation.pause();
          send_progress(&self.simulation, &mut outgoing_events).await?;
        }
        Event::Resume => {
          self.simulation.resume();
          send_progress(&self.simulation, &mut outgoing_events).await?;
          send_event(Event::Step, &mut outgoing_feedback_loop).await?;
        }
        _ => (),
      }
    }

    Ok(())
  }
}

fn message_from_event<E>(event: Event) -> impl Stream<Item = Result<Message, E>>
where
  E: Error + Send + Sync + 'static,
{
  let maybe_message = serde_json::to_string(&event)
    .map(Message::Text)
    .map(Result::Ok)
    .ok();

  stream::iter(maybe_message.into_iter())
}

async fn event_from_try_message(try_message: Result<Message>) -> Option<Event> {
  try_message.ok().and_then(event_from_message)
}

fn event_from_message(message: Message) -> Option<Event> {
  message
    .to_text()
    .ok()
    .and_then(|text| serde_json::from_str::<Event>(text).ok())
}

async fn send_progress<S, E>(simulation: &Simulation, outbound: S) -> Result<()>
where
  S: Sink<Event, Error = E> + Unpin,
  E: Error + Send + Sync + 'static,
{
  let progress = Event::Progress {
    running: simulation.is_running(),
    time: simulation.get_time(),
    levels: Vec::from(simulation.get_levels()), // TODO optimize to avoid copy
  };
  send_event(progress, outbound).await
}

async fn send_event<S, E>(event: Event, mut outbound: S) -> Result<()>
where
  S: Sink<Event, Error = E> + Unpin,
  E: Error + Send + Sync + 'static,
{
  log::info!("Send: {:?}", event);
  outbound.send(event).await?;
  Ok(())
}

async fn send_event_delayed<S, E>(event: Event, outbound: S, delay_millis: u64) -> Result<()>
where
  S: Sink<Event, Error = E> + Unpin,
  E: Error + Send + Sync + 'static,
{
  sleep(Duration::from_millis(delay_millis)).await;
  send_event(event, outbound).await
}

#[cfg(test)]
mod tests {
  use assert_approx_eq::assert_approx_eq;
  use futures::Future;
  use futures_channel::mpsc::{self, Receiver, Sender};

  use super::*;
  use crate::simulation::{
    tests::{assert_slice_approx_eq, assert_slice_approx_eq_with_epsilon},
    DELTA_TIME,
  };

  #[tokio::test]
  async fn protocol_start() {
    with_context(|mut context| async move {
      context.send_incoming_message(Event::Start {
        hours: 4.0,
        landscape: vec![1.0, 2.0],
      });

      sleep(Duration::from_millis(STEP_DELAY_MILLIS - 1)).await;

      context.expect_feedback_empty();

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|running, time, levels| {
        assert!(running);
        assert_approx_eq!(time, 0.0);
        assert_slice_approx_eq(levels.as_slice(), &[1.0, 2.0])
      });

      context.expect_feedback_with(|event| {
        assert_eq!(event, Event::Step);
      })
    })
    .await

    // feedback_loop_rx.map(Result::Ok).forward(feedback_loop_tx);
  }

  #[tokio::test]
  async fn protocol_step() {
    with_context(|mut context| async move {
      context.send_incoming_message(Event::Start {
        hours: DELTA_TIME * 2.0,
        landscape: vec![1.0, 4.0],
      });

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|running, time, levels| {
        assert!(running);
        assert_approx_eq!(time, 0.0);
        assert_slice_approx_eq(levels.as_slice(), &[1.0, 4.0])
      });

      context.expect_feedback_with(|event| {
        assert_eq!(event, Event::Step);
      });

      context.send_feedback(Event::Step);

      sleep(Duration::from_millis(STEP_DELAY_MILLIS - 1)).await;

      context.expect_progress_with(|running, time, levels| {
        assert!(running);
        assert_approx_eq!(time, DELTA_TIME);
        assert_slice_approx_eq_with_epsilon(levels.as_slice(), &[1.16, 3.93], 0.01)
      });

      context.expect_feedback_empty();

      sleep(Duration::from_millis(500)).await;

      context.expect_feedback_with(|event| {
        assert_eq!(event, Event::Step);
      });

      context.send_feedback(Event::Step);

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|running, time, levels| {
        assert!(!running);
        assert_approx_eq!(time, DELTA_TIME * 2.0);
        assert_slice_approx_eq_with_epsilon(levels.as_slice(), &[1.31, 3.88], 0.01)
      });

      context.expect_feedback_empty();
    })
    .await
  }

  #[tokio::test]
  async fn protocol_forward() {
    with_context(|mut context| async move {
      context.send_incoming_message(Event::Start {
        hours: 4.0,
        landscape: vec![1.0, 4.0],
      });

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|_, _, _| ());
      context.expect_feedback_with(|_| ());

      context.send_incoming_message(Event::Forward);

      sleep(Duration::from_millis(10)).await;

      context.expect_progress_with(|running, time, _| {
        assert!(running);
        assert_approx_eq!(time, 0.0);
      });

      context.expect_feedback_with(|event| {
        assert_eq!(event, Event::ForwardStep);
      });
    })
    .await
  }

  #[tokio::test]
  async fn protocol_forward_step() {
    with_context(|mut context| async move {
      context.send_incoming_message(Event::Start {
        hours: FORWARD_HOURS * 2.0,
        landscape: vec![1.0, 4.0],
      });

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|_, _, _| ());
      context.expect_feedback_with(|_| ());

      context.send_incoming_message(Event::Forward);

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|_, _, _| ());
      context.expect_feedback_with(|_| ());

      context.send_feedback(Event::ForwardStep);

      sleep(Duration::from_millis(10)).await;

      context.expect_progress_with(|running, time, _| {
        assert!(running);
        assert_approx_eq!(time, FORWARD_HOURS, 0.1);
      });

      context.expect_feedback_with(|event| {
        assert_eq!(event, Event::ForwardStep);
      });

      context.send_feedback(Event::ForwardStep);

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|running, time, _| {
        assert!(!running);
        assert_approx_eq!(time, FORWARD_HOURS * 2.0, 0.1);
      });

      context.expect_feedback_empty();
    })
    .await
  }

  #[tokio::test]
  async fn protocol_pause() {
    with_context(|mut context| async move {
      context.send_incoming_message(Event::Start {
        hours: 4.0,
        landscape: vec![1.0, 4.0],
      });

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|_, _, _| ());
      context.expect_feedback_with(|_| ());

      context.send_incoming_message(Event::Pause);

      sleep(Duration::from_millis(500)).await;

      context.expect_progress_with(|running, _, _| {
        assert!(!running);
      });

      context.expect_feedback_empty();

      context.send_incoming_message(Event::Resume);

      sleep(Duration::from_millis(10)).await;

      context.expect_progress_with(|running, _, _| {
        assert!(running);
      });

      context.expect_feedback_with(|event| {
        assert_eq!(event, Event::Step);
      });
    })
    .await
  }

  async fn with_context<F, FT, T>(mut f: F) -> T
  where
    F: FnMut(Context) -> FT,
    FT: Future<Output = T>,
  {
    const CHANNEL_SIZE: usize = 32;

    let (outgoing_messages, messages_rx) = mpsc::channel::<Message>(CHANNEL_SIZE);
    let (messages_tx, incoming_messages) = mpsc::channel::<Result<Message, WsError>>(CHANNEL_SIZE);

    let (outgoing_feedback_loop, feedback_loop_rx) = mpsc::channel::<Event>(CHANNEL_SIZE);
    let (feedback_loop_tx, incoming_feedback_loop) = mpsc::channel::<Event>(CHANNEL_SIZE);

    tokio::spawn(async {
      Protocol::new()
        .run(
          outgoing_messages,
          incoming_messages,
          outgoing_feedback_loop,
          incoming_feedback_loop,
        )
        .await
    });

    f(Context::new(
      messages_tx,
      messages_rx,
      feedback_loop_tx,
      feedback_loop_rx,
    ))
    .await
  }

  struct Context {
    message_tx: Sender<Result<Message, WsError>>,
    message_rx: Receiver<Message>,
    feedback_loop_tx: Sender<Event>,
    feedback_loop_rx: Receiver<Event>,
  }

  impl Context {
    fn new(
      message_tx: Sender<Result<Message, WsError>>,
      message_rx: Receiver<Message>,
      feedback_loop_tx: Sender<Event>,
      feedback_loop_rx: Receiver<Event>,
    ) -> Self {
      Self {
        message_tx,
        message_rx,
        feedback_loop_tx,
        feedback_loop_rx,
      }
    }

    fn send_incoming_message(&mut self, event: Event) {
      let message = serde_json::to_string(&event).map(Message::Text).unwrap();
      self.message_tx.try_send(Ok(message)).unwrap();
    }

    fn expect_progress_with<F>(&mut self, f: F)
    where
      F: Fn(bool, f64, Vec<f64>),
    {
      let event = self
        .message_rx
        .try_next()
        .ok()
        .flatten()
        .and_then(|message| {
          message
            .to_text()
            .ok()
            .and_then(|text| serde_json::from_str::<Event>(text).ok())
        });

      match event {
        Some(event) => {
          if let Event::Progress {
            running,
            time,
            levels,
          } = event
          {
            f(running, time, levels)
          } else {
            panic!("Expected progress, but found {:?}", event);
          }
        }
        None => panic!("Expected progress, but nothing found"),
      }
    }

    fn expect_feedback_with<F>(&mut self, f: F)
    where
      F: Fn(Event),
    {
      match self.receive_feedback() {
        Some(event) => f(event),
        None => panic!("Expected feedback, but nothing found"),
      }
    }

    fn expect_feedback_empty(&mut self) {
      if let Some(event) = self.receive_feedback() {
        panic!("Expected no feedback, but found {:?}", event);
      }
    }

    fn send_feedback(&mut self, event: Event) {
      self.feedback_loop_tx.try_send(event).unwrap();
    }

    fn receive_feedback(&mut self) -> Option<Event> {
      self.feedback_loop_rx.try_next().ok().flatten()
    }
  }
}
