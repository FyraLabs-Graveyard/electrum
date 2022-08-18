// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;

use smithay::{
    delegate_output,
    utils::{Logical, Rectangle, Transform},
    wayland::{output::Output, seat::Seat},
};

use super::{CommonState, State};

pub trait OutputExt {
    fn geometry(&self) -> Rectangle<i32, Logical>;
}

pub struct ActiveOutput(pub RefCell<Output>);

impl OutputExt for Output {
    fn geometry(&self) -> Rectangle<i32, Logical> {
        Rectangle::from_loc_and_size(self.current_location(), {
            Transform::from(self.current_transform())
                .transform_size(
                    self.current_mode()
                        .map(|m| m.size)
                        .unwrap_or_else(|| (0, 0).into()),
                )
                .to_f64()
                .to_logical(self.current_scale().fractional_scale())
                .to_i32_round()
        })
    }
}

pub fn active_output(seat: &Seat<State>, state: &CommonState) -> Output {
    seat.user_data()
        .get::<ActiveOutput>()
        .map(|x| x.0.borrow().clone())
        .unwrap_or_else(|| {
            state
                .shell
                .outputs()
                .next()
                .cloned()
                .expect("Backend has no outputs?")
        })
}

pub fn set_active_output(seat: &Seat<State>, output: &Output) {
    if !seat
        .user_data()
        .insert_if_missing(|| ActiveOutput(RefCell::new(output.clone())))
    {
        *seat
            .user_data()
            .get::<ActiveOutput>()
            .unwrap()
            .0
            .borrow_mut() = output.clone();
    }
}

delegate_output!(State);
