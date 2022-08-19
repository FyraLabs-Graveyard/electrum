// SPDX-License-Identifier: GPL-3.0-only

use smithay::{
    delegate_seat,
    wayland::seat::{Seat, SeatHandler, SeatState},
};

use crate::input::SeatId;

use super::State;

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.common.seat_state
    }
}

pub trait SeatExt {
    fn id(&self) -> usize;
}

impl SeatExt for Seat<State> {
    fn id(&self) -> usize {
        self.user_data().get::<SeatId>().unwrap().0
    }
}

delegate_seat!(State);
