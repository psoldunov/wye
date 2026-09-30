//! Event handlers: every data-control object hands its events to
//! [`Clip`]. The ext and wlr protocols get the same handlers through one
//! macro.

use wayland_client::protocol::{wl_callback, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop, event_created_child};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1, ext_data_control_manager_v1, ext_data_control_offer_v1,
    ext_data_control_source_v1,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1, zwlr_data_control_manager_v1, zwlr_data_control_offer_v1,
    zwlr_data_control_source_v1,
};

use super::Clip;
use super::protocol::{Offer, Source};

impl Dispatch<wl_registry::WlRegistry, ()> for Clip {
    fn event(
        state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            state.globals.push((name, interface, version));
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for Clip {
    fn event(
        state: &mut Self,
        _callback: &wl_callback::WlCallback,
        _event: wl_callback::Event,
        (): &(),
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
    ) {
        state.synced = true;
    }
}

delegate_noop!(Clip: ignore wl_seat::WlSeat);
delegate_noop!(Clip: ext_data_control_manager_v1::ExtDataControlManagerV1);
delegate_noop!(Clip: zwlr_data_control_manager_v1::ZwlrDataControlManagerV1);

/// The device, offer and source handlers for one protocol.
macro_rules! data_control {
    ($variant:ident, $device:ident :: $device_ty:ident, $offer:ident :: $offer_ty:ident, $source:ident :: $source_ty:ident) => {
        impl Dispatch<$device::$device_ty, ()> for Clip {
            fn event(
                state: &mut Self,
                _device: &$device::$device_ty,
                event: $device::Event,
                (): &(),
                _connection: &Connection,
                _queue: &QueueHandle<Self>,
            ) {
                match event {
                    $device::Event::DataOffer { id } => state.data_offer(&Offer::$variant(id)),
                    $device::Event::Selection { id } => state.selection(id.map(Offer::$variant)),
                    // Only the regular clipboard is read; primary-selection
                    // offers are dropped at once.
                    $device::Event::PrimarySelection { id: Some(offer) } => {
                        state.forget(&Offer::$variant(offer));
                    }
                    $device::Event::Finished => state.finished = true,
                    _ => {}
                }
            }

            event_created_child!(Clip, $device::$device_ty, [
                $device::EVT_DATA_OFFER_OPCODE => ($offer::$offer_ty, ()),
            ]);
        }

        impl Dispatch<$offer::$offer_ty, ()> for Clip {
            fn event(
                state: &mut Self,
                offer: &$offer::$offer_ty,
                event: $offer::Event,
                (): &(),
                _connection: &Connection,
                _queue: &QueueHandle<Self>,
            ) {
                if let $offer::Event::Offer { mime_type } = event {
                    state.mime(&Offer::$variant(offer.clone()), mime_type);
                }
            }
        }

        impl Dispatch<$source::$source_ty, ()> for Clip {
            fn event(
                state: &mut Self,
                source: &$source::$source_ty,
                event: $source::Event,
                (): &(),
                _connection: &Connection,
                _queue: &QueueHandle<Self>,
            ) {
                let source = Source::$variant(source.clone());
                match event {
                    $source::Event::Send { mime_type: _, fd } => state.send(&source, fd),
                    $source::Event::Cancelled => state.cancelled(&source),
                    _ => {}
                }
            }
        }
    };
}

data_control!(
    Ext,
    ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::ExtDataControlSourceV1
);
data_control!(
    Wlr,
    zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    zwlr_data_control_source_v1::ZwlrDataControlSourceV1
);
