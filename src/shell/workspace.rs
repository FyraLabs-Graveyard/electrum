// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;

use calloop::channel::Sender;
use smithay::{
    desktop::{Kind, Space, Window},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel::{self, ResizeEdge},
        wayland_server::DisplayHandle,
    },
    utils::IsAlive,
    wayland::{
        output::Output,
        seat::{PointerGrabStartData, Seat},
        Serial,
    },
};

use crate::{runtime::messages::RuntimeMessage, state::State};

use super::layout::Layout;

pub struct Workspace {
    pub idx: u8,
    pub space: Space,
    pub fullscreen: HashMap<String, Window>,
    pub runtime_sender: Sender<RuntimeMessage>,
    pub layer: Layout,
}

impl Workspace {
    pub fn new(idx: u8, rs: Sender<RuntimeMessage>) -> Self {
        Self {
            idx,
            space: Space::new(slog_scope::logger()),
            fullscreen: HashMap::new(),
            runtime_sender: rs,
            layer: Layout::new(),
        }
    }

    pub fn refresh(&mut self, dh: &DisplayHandle) {
        let outputs = self.space.outputs().collect::<Vec<_>>();
        let dead_windows = self
            .fullscreen
            .iter()
            .filter(|(name, _)| !outputs.iter().any(|o| o.name() == **name))
            .map(|(_, w)| w)
            .cloned()
            .collect::<Vec<_>>();
        for window in dead_windows {
            self.unfullscreen_request(&window);
        }
        self.fullscreen.retain(|_, w| w.alive());
        self.space.refresh(dh);
    }

    /// Deno Function
    pub fn maximize_request(&mut self, window: &Window, output: &Output) {
        if self.fullscreen.values().any(|w| w == window) {
            return;
        }

        self.runtime_sender
            .send(RuntimeMessage::MaximizeRequest {
                window: window.clone(),
                output: output.clone(),
            })
            .unwrap();
    }

    /// Deno Function
    pub fn unmaximize_request(&mut self, window: &Window) {
        if self.fullscreen.values().any(|w| w == window) {
            return self.unfullscreen_request(window);
        }

        self.runtime_sender
            .send(RuntimeMessage::UnmaximizeRequest {
                window: window.clone(),
            })
            .unwrap();
    }

    /// Deno Function
    pub fn resize_request(
        &mut self,
        window: &Window,
        seat: &Seat<State>,
        serial: Serial,
        start_data: PointerGrabStartData,
        edges: ResizeEdge,
    ) {
        if self.fullscreen.values().any(|w| w == window) {
            return;
        }

        self.runtime_sender
            .send(RuntimeMessage::ResizeRequest {
                window: window.clone(),
                seat: seat.clone(),
                serial,
                start_data,
                edges,
            })
            .unwrap();
    }

    pub fn fullscreen_request(&mut self, window: &Window, output: &Output) {
        if self.fullscreen.contains_key(&output.name()) {
            return;
        }

        #[allow(irrefutable_let_patterns)]
        if let Kind::Xdg(xdg) = &window.toplevel() {
            xdg.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Fullscreen);
                state.size = Some(
                    output
                        .current_mode()
                        .map(|m| m.size)
                        .unwrap_or((0, 0).into())
                        .to_f64()
                        .to_logical(output.current_scale().fractional_scale())
                        .to_i32_round(),
                );
            });

            xdg.send_configure();
            self.fullscreen.insert(output.name(), window.clone());
        }
    }

    /// Deno Function
    pub fn unfullscreen_request(&mut self, window: &Window) {
        if self.fullscreen.values().any(|w| w == window) {
            #[allow(irrefutable_let_patterns)]
            if let Kind::Xdg(xdg) = &window.toplevel() {
                xdg.with_pending_state(|state| {
                    state.states.unset(xdg_toplevel::State::Fullscreen);
                    state.size = None;
                });
                xdg.send_configure();
            }

            self.runtime_sender
                .send(RuntimeMessage::UnfullscreenRequest {
                    window: window.clone(),
                })
                .unwrap();

            self.fullscreen.retain(|_, w| w != window);
        }
    }

    pub fn get_fullscreen(&self, output: &Output) -> Option<&Window> {
        if !self.space.outputs().any(|o| o == output) {
            return None;
        }

        self.fullscreen.get(&output.name()).filter(|w| w.alive())
    }
}
