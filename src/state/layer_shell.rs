// SPDX-License-Identifier: GPL-3.0-only

use smithay::{
    delegate_layer_shell,
    desktop::{LayerSurface, PopupKind},
    reexports::wayland_server::{protocol::wl_output::WlOutput, DisplayHandle},
    wayland::{
        output::Output,
        shell::{
            wlr_layer::{
                Layer, LayerSurface as WlrLayerSurface, WlrLayerShellHandler, WlrLayerShellState,
            },
            xdg::PopupSurface,
        },
    },
};

use super::{output::active_output, State};

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.common.shell.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        _dh: &DisplayHandle,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let seat = self.common.last_active_seat.clone();
        let output = output
            .as_ref()
            .and_then(Output::from_resource)
            .unwrap_or(active_output(&seat, &self.common));
        self.common.shell.pending_layers.push((
            LayerSurface::new(surface, namespace),
            output,
            seat,
        ));
    }

    fn new_popup(&mut self, _dh: &DisplayHandle, _parent: WlrLayerSurface, popup: PopupSurface) {
        let positioner = popup.with_pending_state(|state| state.positioner);
        self.common.shell.unconstrain_popup(&popup, &positioner);

        if popup.send_configure().is_ok() {
            self.common
                .shell
                .popups
                .track_popup(PopupKind::from(popup))
                .unwrap();
        }
    }
}

delegate_layer_shell!(State);
