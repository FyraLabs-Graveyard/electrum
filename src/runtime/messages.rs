use smithay::{
    desktop::Window,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge,
    wayland::{
        output::Output,
        seat::{PointerGrabStartData, Seat},
        Serial,
    },
};

use crate::state::State;

// Messages from the runtime to the compositor
pub enum RuntimeMessage {
    Ping,
    MaximizeRequest {
        window: Window,
        output: Output,
    },
    UnmaximizeRequest {
        window: Window,
    },
    ResizeRequest {
        window: Window,
        seat: Seat<State>,
        serial: Serial,
        start_data: PointerGrabStartData,
        edges: ResizeEdge,
    },
    UnfullscreenRequest {
        window: Window,
    },
}

// Messages from the compositor to the runtime
pub enum CompositorMessage {
    Ping,
}
