// SPDX-License-Identifier: GPL-3.0-only

use std::{ffi::OsString, time::Instant};

use smithay::{
    reexports::{
        calloop::{channel::Sender, LoopHandle, LoopSignal},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            Display, DisplayHandle,
        },
    },
    wayland::{
        compositor::CompositorState,
        data_device::DataDeviceState,
        dmabuf::DmabufState,
        output::OutputManagerState,
        primary_selection::PrimarySelectionState,
        seat::{Seat, SeatState},
        shm::ShmState,
        viewporter::ViewporterState,
    },
};

use crate::{
    backend::winit::state::WinitState, input::add_seat, log::LogState,
    runtime::messages::RuntimeMessage, shell::Shell,
};

mod buffer;
mod compositor;
mod data_device;
mod dmabuf;
mod layer_shell;
pub(crate) mod output;
mod primary_selection;
pub(crate) mod seat;
mod shm;
mod viewporter;
pub(crate) mod xdg_shell;

pub enum BackendData {
    Winit(WinitState),
    Unset,
}

impl BackendData {
    pub fn winit(&mut self) -> &mut WinitState {
        match self {
            BackendData::Winit(ref mut winit_state) => winit_state,
            _ => unreachable!("Called winit() in non-winit backend"),
        }
    }
}

pub struct ClientState {}
impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

pub struct Data {
    pub display: Display<State>,
    pub state: State,
}

pub type LoopData = Data;

pub struct State {
    pub backend: BackendData,
    pub common: CommonState,
}

pub struct CommonState {
    pub socket: OsString,
    pub event_loop_handle: LoopHandle<'static, LoopData>,
    pub event_loop_signal: LoopSignal,

    pub shell: Shell,
    pub seats: Vec<Seat<State>>,
    pub last_active_seat: Seat<State>,

    pub start_time: Instant,
    pub should_stop: bool,
    pub log: LogState,

    // Wayland State
    pub compositor_state: CompositorState,
    pub data_device_state: DataDeviceState,
    pub dmabuf_state: DmabufState,
    pub output_state: OutputManagerState,
    pub primary_selection_state: PrimarySelectionState,
    pub seat_state: SeatState<State>,
    pub shm_state: ShmState,
    pub viewporter_state: ViewporterState,
}

impl State {
    pub fn new(
        dh: &DisplayHandle,
        socket: OsString,
        handle: LoopHandle<'static, LoopData>,
        signal: LoopSignal,
        log: LogState,
        runtime_sender: Sender<RuntimeMessage>,
    ) -> Self {
        let initial_seat = add_seat(dh, "seat-0".to_string());

        Self {
            backend: BackendData::Unset,
            common: CommonState {
                socket,
                event_loop_handle: handle,
                event_loop_signal: signal,

                // TODO: Have input managers handle this
                shell: Shell::new(&dh, runtime_sender),
                seats: vec![initial_seat.clone()],
                last_active_seat: initial_seat,

                start_time: Instant::now(),
                should_stop: false,
                log,

                compositor_state: CompositorState::new::<Self, _>(dh, slog_scope::logger()),
                data_device_state: DataDeviceState::new::<Self, _>(dh, slog_scope::logger()),
                dmabuf_state: DmabufState::new(),
                primary_selection_state: PrimarySelectionState::new::<Self, _>(
                    dh,
                    slog_scope::logger(),
                ),
                output_state: OutputManagerState::new_with_xdg_output::<Self>(dh),
                seat_state: SeatState::<Self>::new(),
                shm_state: ShmState::new::<Self, _>(dh, vec![], slog_scope::logger()),
                viewporter_state: ViewporterState::new::<Self, _>(dh, slog_scope::logger()),
            },
        }
    }

    pub fn new_client_state(&self) -> ClientState {
        ClientState {}
    }
}
