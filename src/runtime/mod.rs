use std::env::{self, current_dir};
use std::path::PathBuf;

use crate::LoopData;
use calloop::channel::{channel, Channel, Event, Sender};
use calloop::futures::{Executor, Scheduler};
use calloop::EventLoop;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_runtime::worker::MainWorker;

mod main;
pub mod messages;
mod module;
mod extension;

use futures::channel::mpsc::UnboundedSender;
use messages::{CompositorMessage, RuntimeMessage};

pub struct Runtime {
    main_worker: MainWorker,
    main_module: ModuleSpecifier,
    runtime_channel: Channel<RuntimeMessage>,
    compositor_sender: Sender<CompositorMessage>,
    event_sender: UnboundedSender<extension::Event>,

    pub runtime_sender: Sender<RuntimeMessage>,
}

impl Runtime {
    pub fn new(compositor_sender: Sender<CompositorMessage>) -> Self {
        let (runtime_sender, runtime_channel) = channel::<RuntimeMessage>();
        let mut config_path: PathBuf;
        if cfg!(feature = "devel") {
            config_path = match env::var("TS_PREFIX") {
                Ok(x) => PathBuf::from(x),
                Err(_) => current_dir().unwrap(),
            };
            config_path.push("src");
            config_path.push("ts");
            config_path.push("main.ts");
        } else {
            let xdg_dirs = xdg::BaseDirectories::with_prefix("electrum").unwrap();
            config_path = xdg_dirs.get_config_file("main.ts");
        }

        let main_module = deno_core::resolve_path(config_path.to_str().unwrap())
            .expect("failed to resolve main module");
        let main_worker_instance = main::new(main_module.clone());

        Runtime {
            main_worker: main_worker_instance.worker,
            event_sender: main_worker_instance.event_sender,
            main_module,
            runtime_channel,
            runtime_sender,
            compositor_sender,
        }
    }

    pub fn run_with_calloop(mut self, event_loop: &mut EventLoop<LoopData>) {
        let (exec, sched): (
            Executor<Result<(), AnyError>>,
            Scheduler<Result<(), AnyError>>,
        ) = calloop::futures::executor().unwrap();

        event_loop
            .handle()
            .insert_source(exec, |evt, _metadata, _shared| {
                evt.unwrap();
            })
            .unwrap();

        let compositor_sender = self.compositor_sender.clone();

        event_loop
            .handle()
            .insert_source(
                self.runtime_channel,
                move |message, _metadata, _shared| match message {
                    Event::Msg(RuntimeMessage::Ping) => {
                        slog_scope::info!("The runtime got a ping!");
                        compositor_sender.send(CompositorMessage::Ping).unwrap();
                    }
                    Event::Msg(_) => todo!(),
                    Event::Closed => todo!(),
                },
            )
            .unwrap();

        sched
            .schedule(async move {
                self.main_worker
                    .execute_main_module(&self.main_module)
                    .await
                    .unwrap();
                self.main_worker.run_event_loop(false).await
            })
            .unwrap();
    }
}
