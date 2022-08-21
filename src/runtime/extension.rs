use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde::Serialize;
use deno_core::{Extension, op, OpState};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::StreamExt;

#[derive(Serialize, Debug)]
pub enum Event {
  Ping
}

#[op]
pub async fn op_electrum_poll_events(state: &mut OpState) -> Result<Option<Event>, AnyError> {
  let mut channel = state.borrow_mut::<Rc<RefCell<UnboundedReceiver<Event>>>>().try_borrow_mut()?;
  let val = channel.next().await;
  println!("{:?}", val);
  Ok(val)
}

pub struct MainExtensionInstance {
  pub extension: Extension,
  pub event_sender: UnboundedSender<Event>
}

pub fn main_extension() -> MainExtensionInstance {
  let (sender, reciever) = unbounded();
  let reciever = Rc::new(RefCell::new(reciever));
  let extension = Extension::builder()
    .state(move |state| {
        state.put(reciever.clone());
        Ok(())
      })
      .ops(vec![op_electrum_poll_events::decl()])
      .build();
    
      MainExtensionInstance {
        extension,
        event_sender: sender
      }
}