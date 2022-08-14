// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;

use smithay::{
    delegate_data_device,
    reexports::wayland_server::protocol::{wl_data_source::WlDataSource, wl_surface::WlSurface},
    wayland::{
        data_device::{
            ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
        },
        seat::Seat,
    },
};

use super::State;

// TODO: Should this be in Deno?
pub struct DnDIcon {
    surface: RefCell<Option<WlSurface>>,
}

impl ClientDndGrabHandler for State {
    fn started(
        &mut self,
        _source: Option<WlDataSource>,
        icon: Option<WlSurface>,
        seat: Seat<Self>,
    ) {
        let user_data = seat.user_data();
        user_data.insert_if_missing(|| DnDIcon {
            surface: RefCell::new(None),
        });
        *user_data.get::<DnDIcon>().unwrap().surface.borrow_mut() = icon;
    }
    fn dropped(&mut self, seat: Seat<Self>) {
        seat.user_data()
            .get::<DnDIcon>()
            .unwrap()
            .surface
            .borrow_mut()
            .take();
    }
}
impl ServerDndGrabHandler for State {}
impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.common.data_device_state
    }
}

delegate_data_device!(State);
