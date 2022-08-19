// SPDX-License-Identifier: GPL-3.0-only

use smithay::backend::input::{
    Axis, AxisSource, ButtonState, Device, DeviceCapability, Event, InputBackend, InputEvent,
    PointerAxisEvent, PointerButtonEvent, PointerMotionAbsoluteEvent, PointerMotionEvent,
};

use smithay::desktop::{layer_map_for_output, WindowSurfaceType};
use smithay::reexports::wayland_server::protocol::wl_pointer::{self};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::output::Output;
use smithay::wayland::seat::{AxisFrame, ButtonEvent, CursorImageStatus, MotionEvent, Seat};
use smithay::wayland::shell::wlr_layer::Layer as WlrLayer;
use smithay::wayland::SERIAL_COUNTER;
use std::cell::RefCell;
use std::collections::HashMap;

use crate::id::id_gen;
use crate::shell::grab::SeatMoveGrabState;
use crate::shell::workspace::Workspace;
use crate::state::output::{active_output, set_active_output, OutputExt};
use crate::state::State;

id_gen!(next_seat_id, SEAT_ID, SEAT_IDS);

#[repr(transparent)]
pub struct SeatId(pub usize);

#[derive(Default)]
pub struct SupressedKeys(RefCell<Vec<u32>>);
#[derive(Default)]
pub struct Devices(RefCell<HashMap<String, Vec<DeviceCapability>>>);

impl Default for SeatId {
    fn default() -> SeatId {
        SeatId(next_seat_id())
    }
}

impl Drop for SeatId {
    fn drop(&mut self) {
        SEAT_IDS.lock().unwrap().remove(&self.0);
    }
}

impl Devices {
    fn add_device<D: Device>(&self, device: &D) -> Vec<DeviceCapability> {
        let id = device.id();
        let mut map = self.0.borrow_mut();
        let caps = [DeviceCapability::Keyboard, DeviceCapability::Pointer]
            .iter()
            .cloned()
            .filter(|c| device.has_capability(*c))
            .collect::<Vec<_>>();
        let new_caps = caps
            .iter()
            .cloned()
            .filter(|c| map.values().flatten().all(|has| *c != *has))
            .collect::<Vec<_>>();
        map.insert(id, caps);
        new_caps
    }

    fn remove_device<D: Device>(&self, device: &D) -> Vec<DeviceCapability> {
        let id = device.id();
        let mut map = self.0.borrow_mut();
        map.remove(&id)
            .unwrap_or(Vec::new())
            .into_iter()
            .filter(|c| map.values().flatten().all(|has| *c != *has))
            .collect()
    }

    pub fn has_device<D: Device>(&self, device: &D) -> bool {
        self.0.borrow().contains_key(&device.id())
    }
}

pub fn add_seat(dh: &DisplayHandle, name: String) -> Seat<State> {
    let mut seat = Seat::<State>::new(dh, name, None);
    let userdata = seat.user_data();
    // userdata.insert_if_missing(SeatId::default);
    userdata.insert_if_missing(Devices::default);
    userdata.insert_if_missing(SupressedKeys::default);
    userdata.insert_if_missing(SeatMoveGrabState::default);
    userdata.insert_if_missing(|| RefCell::new(CursorImageStatus::Default));

    let owned_seat = seat.clone();
    seat.add_pointer(move |status| {
        *owned_seat
            .user_data()
            .get::<RefCell<CursorImageStatus>>()
            .unwrap()
            .borrow_mut() = status;
    });

    seat
}

impl State {
    pub fn process_input_event<B: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        event: InputEvent<B>,
    ) {
        match event {
            InputEvent::DeviceAdded { device } => {
                let seat = &mut self.common.last_active_seat;
                let userdata = seat.user_data();
                let devices = userdata.get::<Devices>().unwrap();
                for cap in devices.add_device(&device) {
                    match cap {
                        // TODO: Handle touch, tablet
                        _ => {}
                    }
                }
            }
            InputEvent::DeviceRemoved { device } => {
                for seat in &mut self.common.seats {
                    let userdata = seat.user_data();
                    let devices = userdata.get::<Devices>().unwrap();
                    if devices.has_device(&device) {
                        for cap in devices.remove_device(&device) {
                            match cap {
                                // TODO: Handle touch, tablet
                                _ => {}
                            }
                        }
                        break;
                    }
                }
            }
            InputEvent::Keyboard { event: _ } => {}
            InputEvent::PointerMotion { event } => {
                let device = event.device();
                for seat in self.common.seats.clone().iter() {
                    let userdata = seat.user_data();
                    let devices = userdata.get::<Devices>().unwrap();
                    if devices.has_device(&device) {
                        let current_output = active_output(seat, &self.common);

                        let mut position = seat.get_pointer().unwrap().current_location();
                        position += event.delta();

                        let output = self
                            .common
                            .shell
                            .outputs()
                            .find(|output| output.geometry().to_f64().contains(position))
                            .cloned()
                            .unwrap_or(current_output.clone());
                        if output != current_output {
                            set_active_output(seat, &output);
                        }
                        let output_geometry = output.geometry();

                        position.x = 0.0f64
                            .max(position.x)
                            .min((output_geometry.loc.x + output_geometry.size.w) as f64);
                        position.y = 0.0f64
                            .max(position.y)
                            .min((output_geometry.loc.y + output_geometry.size.h) as f64);

                        let serial = SERIAL_COUNTER.next_serial();
                        let relative_pos = self
                            .common
                            .shell
                            .space_relative_output_geometry(position, &output);
                        let workspace = self.common.shell.active_workspace_mut();
                        let under = State::surface_under(
                            position,
                            relative_pos,
                            &output,
                            output_geometry,
                            &workspace,
                        );
                        seat.get_pointer().unwrap().motion(
                            self,
                            dh,
                            &MotionEvent {
                                location: position,
                                focus: under,
                                serial,
                                time: event.time(),
                            },
                        );

                        break;
                    }
                }
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let device = event.device();
                for seat in self.common.seats.clone().iter() {
                    let userdata = seat.user_data();
                    let devices = userdata.get::<Devices>().unwrap();
                    if devices.has_device(&device) {
                        let output = active_output(seat, &self.common);
                        let geometry = output.geometry();
                        let position =
                            geometry.loc.to_f64() + event.position_transformed(geometry.size);
                        let relative_pos = self
                            .common
                            .shell
                            .space_relative_output_geometry(position, &output);
                        let workspace = self.common.shell.active_workspace_mut();
                        let serial = SERIAL_COUNTER.next_serial();
                        let under = State::surface_under(
                            position,
                            relative_pos,
                            &output,
                            geometry,
                            &workspace,
                        );
                        seat.get_pointer().unwrap().motion(
                            self,
                            dh,
                            &MotionEvent {
                                location: position,
                                focus: under,
                                serial,
                                time: event.time(),
                            },
                        );
                        break;
                    }
                }
            }
            InputEvent::PointerButton { event } => {
                let device = event.device();
                for seat in self.common.seats.clone().iter() {
                    let userdata = seat.user_data();
                    let devices = userdata.get::<Devices>().unwrap();
                    if devices.has_device(&device) {
                        let serial = SERIAL_COUNTER.next_serial();
                        let button = event.button_code();
                        let state = match event.state() {
                            ButtonState::Pressed => {
                                // change the keyboard focus unless the pointer or keyboard is grabbed
                                // We test for any matching surface type here but always use the root
                                // (in case of a window the toplevel) surface for the focus.
                                // see: https://gitlab.freedesktop.org/wayland/wayland/-/issues/294
                                if !seat.get_pointer().unwrap().is_grabbed()
                                    && !seat.get_keyboard().map(|k| k.is_grabbed()).unwrap_or(false)
                                {
                                    let output = active_output(seat, &self.common);
                                    let pos = seat.get_pointer().unwrap().current_location();
                                    let output_geo = output.geometry();
                                    let relative_pos = self
                                        .common
                                        .shell
                                        .space_relative_output_geometry(pos, &output);
                                    let workspace = self.common.shell.active_workspace_mut();
                                    let layers = layer_map_for_output(&output);
                                    let mut under = None;

                                    if let Some(window) = workspace.get_fullscreen(&output) {
                                        if let Some(layer) =
                                            layers.layer_under(WlrLayer::Overlay, relative_pos)
                                        {
                                            if layer.can_receive_keyboard_focus() {
                                                let layer_loc =
                                                    layers.layer_geometry(layer).unwrap().loc;
                                                under = layer
                                                    .surface_under(
                                                        pos - output_geo.loc.to_f64()
                                                            - layer_loc.to_f64(),
                                                        WindowSurfaceType::ALL,
                                                    )
                                                    .map(|(_, _)| layer.wl_surface().clone());
                                            }
                                        } else {
                                            under = window
                                                .surface_under(
                                                    pos - output_geo.loc.to_f64(),
                                                    WindowSurfaceType::ALL,
                                                )
                                                .map(|(_, _)| {
                                                    window.toplevel().wl_surface().clone()
                                                });
                                        }
                                    } else {
                                        if let Some(layer) = layers
                                            .layer_under(WlrLayer::Overlay, relative_pos)
                                            .or_else(|| {
                                                layers.layer_under(WlrLayer::Top, relative_pos)
                                            })
                                        {
                                            if layer.can_receive_keyboard_focus() {
                                                let layer_loc =
                                                    layers.layer_geometry(layer).unwrap().loc;
                                                under = layer
                                                    .surface_under(
                                                        pos - output_geo.loc.to_f64()
                                                            - layer_loc.to_f64(),
                                                        WindowSurfaceType::ALL,
                                                    )
                                                    .map(|(_, _)| layer.wl_surface().clone());
                                            }
                                        } else if let Some((window, _, _)) = workspace
                                            .space
                                            .surface_under(relative_pos, WindowSurfaceType::ALL)
                                        {
                                            under = Some(window.toplevel().wl_surface().clone());
                                        } else if let Some(layer) =
                                            layers.layer_under(WlrLayer::Bottom, pos).or_else(
                                                || layers.layer_under(WlrLayer::Background, pos),
                                            )
                                        {
                                            if layer.can_receive_keyboard_focus() {
                                                let layer_loc =
                                                    layers.layer_geometry(layer).unwrap().loc;
                                                under = layer
                                                    .surface_under(
                                                        pos - output_geo.loc.to_f64()
                                                            - layer_loc.to_f64(),
                                                        WindowSurfaceType::ALL,
                                                    )
                                                    .map(|(_, _)| layer.wl_surface().clone());
                                            }
                                        };
                                    }

                                    self.common
                                        .set_focus(dh, under.as_ref(), seat, Some(serial));
                                }
                                wl_pointer::ButtonState::Pressed
                            }
                            ButtonState::Released => wl_pointer::ButtonState::Released,
                        };
                        seat.get_pointer().unwrap().button(
                            self,
                            dh,
                            &ButtonEvent {
                                button,
                                state,
                                serial,
                                time: event.time(),
                            },
                        );
                        break;
                    }
                }
            }
            InputEvent::PointerAxis { event } => {
                let device = event.device();
                for seat in self.common.seats.clone().iter() {
                    let userdata = seat.user_data();
                    let devices = userdata.get::<Devices>().unwrap();
                    if devices.has_device(&device) {
                        let source = match event.source() {
                            AxisSource::Continuous => wl_pointer::AxisSource::Continuous,
                            AxisSource::Finger => wl_pointer::AxisSource::Finger,
                            AxisSource::Wheel | AxisSource::WheelTilt => {
                                wl_pointer::AxisSource::Wheel
                            }
                        };
                        let horizontal_amount =
                            event.amount(Axis::Horizontal).unwrap_or_else(|| {
                                event.amount_discrete(Axis::Horizontal).unwrap_or(0.0) * 3.0
                            });
                        let vertical_amount = event.amount(Axis::Vertical).unwrap_or_else(|| {
                            event.amount_discrete(Axis::Vertical).unwrap_or(0.0) * 3.0
                        });
                        let horizontal_amount_discrete = event.amount_discrete(Axis::Horizontal);
                        let vertical_amount_discrete = event.amount_discrete(Axis::Vertical);

                        {
                            let mut frame = AxisFrame::new(event.time()).source(source);
                            if horizontal_amount != 0.0 {
                                frame = frame
                                    .value(wl_pointer::Axis::HorizontalScroll, horizontal_amount);
                                if let Some(discrete) = horizontal_amount_discrete {
                                    frame = frame.discrete(
                                        wl_pointer::Axis::HorizontalScroll,
                                        discrete as i32,
                                    );
                                }
                            } else if source == wl_pointer::AxisSource::Finger {
                                frame = frame.stop(wl_pointer::Axis::HorizontalScroll);
                            }
                            if vertical_amount != 0.0 {
                                frame =
                                    frame.value(wl_pointer::Axis::VerticalScroll, vertical_amount);
                                if let Some(discrete) = vertical_amount_discrete {
                                    frame = frame.discrete(
                                        wl_pointer::Axis::VerticalScroll,
                                        discrete as i32,
                                    );
                                }
                            } else if source == wl_pointer::AxisSource::Finger {
                                frame = frame.stop(wl_pointer::Axis::VerticalScroll);
                            }
                            seat.get_pointer().unwrap().axis(self, dh, frame);
                        }
                        break;
                    }
                }
            }
            InputEvent::TouchDown { event: _ } => {}
            InputEvent::TouchMotion { event: _ } => {}
            InputEvent::TouchUp { event: _ } => {}
            InputEvent::TouchCancel { event: _ } => {}
            InputEvent::TouchFrame { event: _ } => {}
            InputEvent::TabletToolAxis { event: _ } => {}
            InputEvent::TabletToolProximity { event: _ } => {}
            InputEvent::TabletToolTip { event: _ } => {}
            InputEvent::TabletToolButton { event: _ } => {}
            InputEvent::Special(_) => {}
        }
    }

    pub fn surface_under(
        global_pos: Point<f64, Logical>,
        relative_pos: Point<f64, Logical>,
        output: &Output,
        output_geo: Rectangle<i32, Logical>,
        workspace: &Workspace,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        let layers = layer_map_for_output(output);
        if let Some(window) = workspace.get_fullscreen(output) {
            if let Some(layer) = layers
                .layer_under(WlrLayer::Overlay, relative_pos)
                .or_else(|| layers.layer_under(WlrLayer::Top, relative_pos))
            {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        global_pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(s, loc)| (s, loc + layer_loc + output_geo.loc))
            } else {
                window
                    .surface_under(global_pos - output_geo.loc.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, loc)| (s, loc + output_geo.loc))
            }
        } else {
            if let Some(layer) = layers
                .layer_under(WlrLayer::Overlay, relative_pos)
                .or_else(|| layers.layer_under(WlrLayer::Top, relative_pos))
            {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        global_pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(s, loc)| (s, loc + layer_loc + output_geo.loc))
            } else if let Some((_, surface, loc)) = workspace
                .space
                .surface_under(relative_pos, WindowSurfaceType::ALL)
            {
                Some((surface, loc + (global_pos - relative_pos).to_i32_round()))
            } else if let Some(layer) = layers
                .layer_under(WlrLayer::Bottom, relative_pos)
                .or_else(|| layers.layer_under(WlrLayer::Background, relative_pos))
            {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        global_pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(s, loc)| (s, loc + layer_loc + output_geo.loc))
            } else {
                None
            }
        }
    }
}
