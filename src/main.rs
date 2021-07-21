mod protocol;
mod simulation;
mod water_flow;

use std::net::SocketAddr;

use anyhow::Result;
use futures_channel::mpsc;
use futures_util::StreamExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tungstenite::Error as WsError;

use crate::{protocol::Protocol, simulation::Simulation};

const FEEDBACK_CHANNEL_SIZE: usize = 1024;

#[tokio::main]
async fn main() -> Result<()> {
  env_logger::init();

  let port = std::env::var("PORT").unwrap_or_else(|_| "9002".to_string());
  let addr = format!("0.0.0.0:{}", port);
  start_server(addr).await
}

async fn start_server<S: AsRef<str>>(addr: S) -> Result<()> {
  let listener = TcpListener::bind(addr.as_ref()).await?;
  log::info!("Listening on: {}", addr.as_ref());

  while let Ok((stream, _)) = listener.accept().await {
    let peer = stream.peer_addr()?;
    tokio::spawn(accept_connection(peer, stream));
  }

  Ok(())
}

async fn accept_connection(peer: SocketAddr, stream: TcpStream) {
  if let Err(err) = handle_connection(peer, stream).await {
    if let Some(source) = err.source() {
      match source.downcast_ref::<WsError>() {
        Some(WsError::ConnectionClosed) | Some(WsError::Protocol(_)) | Some(WsError::Utf8) => (),
        _ => log::error!("Error processing connection: {}", source),
      }
    }
  }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream) -> Result<()> {
  let messages = accept_async(stream).await?;
  log::info!("New WebSocket connection: {}", peer);

  let (outgoing_messages, incoming_messages) = messages.split();

  let (outgoing_feedback_loop, incoming_feedback_loop) = mpsc::channel(FEEDBACK_CHANNEL_SIZE);

  let simulation = Simulation::new();

  Protocol::new(simulation)
    .run(
      outgoing_messages,
      incoming_messages,
      outgoing_feedback_loop,
      incoming_feedback_loop,
    )
    .await
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use assert_approx_eq::assert_approx_eq;
  use futures::stream::StreamExt;
  use futures::{Future, Sink, SinkExt, Stream, TryStreamExt};
  use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

  use super::*;
  use crate::{protocol::Event, simulation::tests::assert_slice_approx_eq_with_epsilon};

  #[tokio::test]
  async fn successful_connection() {
    with_context(|mut client_events, mut server_events| async move {
      client_events
        .send(Event::Start {
          hours: 1.0,
          landscape: vec![1.0, 2.0],
        })
        .await
        .unwrap();

      let mut counter: usize = 0;
      while let Some(Ok(event)) = server_events.next().await {
        if let Event::Progress {
          running,
          time,
          levels,
        } = event
        {
          if !running {
            assert_approx_eq!(time, 1.0);
            assert_slice_approx_eq_with_epsilon(levels.as_slice(), &[2.5, 2.5], 0.01);
            break;
          } else {
            counter += 1;
          }
        } else {
          panic!("Expected a progress event, but found: {:?}", event);
        }
      }
      assert_eq!(counter, 11);
      client_events.close().await.unwrap();
    })
    .await;
  }

  async fn with_context<F, FT, T>(f: F) -> T
  where
    F: Fn(
      Box<dyn Sink<Event, Error = anyhow::Error> + Unpin>,
      Box<dyn Stream<Item = anyhow::Result<Event>> + Unpin>,
    ) -> FT,
    FT: Future<Output = T>,
  {
    let port: u16 = 9002;
    let addr = format!("127.0.0.1:{}", port);
    tokio::spawn(start_server(addr.clone()));
    tokio::time::sleep(Duration::from_secs(1)).await;

    let url = format!("ws://127.0.0.1:{}", port);
    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
    let (write, read) = ws_stream.split();

    let client_events = write.with(|event: Event| {
      let message = serde_json::to_string::<Event>(&event)
        .map_err(anyhow::Error::from)
        .map(Message::Text);
      futures::future::ready(message)
    });

    let server_events = read.map_err(anyhow::Error::from).map(|try_message| {
      try_message.and_then(|message| match message {
        Message::Text(text) => {
          serde_json::from_str::<Event>(text.as_str()).map_err(anyhow::Error::from)
        }
        _ => Err(anyhow::anyhow!("Unexpected message format")),
      })
    });

    f(Box::new(client_events), Box::new(server_events)).await
  }
}
