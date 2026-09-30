//! The two data-control protocols behind one set of types:
//! `ext-data-control-v1` (the standard one; `KWin` 6 offers only this) and
//! `zwlr-data-control-v1` (wlroots compositors). Both have the same
//! requests and events, so each object is an enum over the two, and the
//! event handlers in `dispatch` feed one [`super::Clip`] state.

use std::os::fd::BorrowedFd;

use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Proxy, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::ExtDataControlDeviceV1,
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::ExtDataControlSourceV1,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
};

use super::Clip;

/// Interface names in the registry.
pub const EXT_MANAGER: &str = "ext_data_control_manager_v1";
pub const WLR_MANAGER: &str = "zwlr_data_control_manager_v1";

/// Mechanism names for `Status.capabilities`.
pub const EXT_MECHANISM: &str = "wayland-ext-data-control";
pub const WLR_MECHANISM: &str = "wayland-wlr-data-control";

/// The bound manager.
#[derive(Debug, Clone)]
pub enum Manager {
    Ext(ExtDataControlManagerV1),
    Wlr(ZwlrDataControlManagerV1),
}

/// The seat's data device.
#[derive(Debug, Clone)]
pub enum Device {
    Ext(ExtDataControlDeviceV1),
    Wlr(ZwlrDataControlDeviceV1),
}

/// Something another client (or Wye) put on the clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    Ext(ExtDataControlOfferV1),
    Wlr(ZwlrDataControlOfferV1),
}

/// What Wye puts on the clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Ext(ExtDataControlSourceV1),
    Wlr(ZwlrDataControlSourceV1),
}

impl Manager {
    pub fn mechanism(&self) -> &'static str {
        match self {
            Self::Ext(_) => EXT_MECHANISM,
            Self::Wlr(_) => WLR_MECHANISM,
        }
    }

    pub fn device(&self, seat: &WlSeat, queue: &QueueHandle<Clip>) -> Device {
        match self {
            Self::Ext(manager) => Device::Ext(manager.get_data_device(seat, queue, ())),
            Self::Wlr(manager) => Device::Wlr(manager.get_data_device(seat, queue, ())),
        }
    }

    pub fn source(&self, queue: &QueueHandle<Clip>) -> Source {
        match self {
            Self::Ext(manager) => Source::Ext(manager.create_data_source(queue, ())),
            Self::Wlr(manager) => Source::Wlr(manager.create_data_source(queue, ())),
        }
    }
}

impl Device {
    pub fn set_selection(&self, source: &Source) {
        match (self, source) {
            (Self::Ext(device), Source::Ext(source)) => device.set_selection(Some(source)),
            (Self::Wlr(device), Source::Wlr(source)) => device.set_selection(Some(source)),
            // One manager makes both, so the kinds always match.
            _ => tracing::warn!("data-control device and source differ; not writing"),
        }
    }
}

impl Offer {
    pub fn receive(&self, mime: String, fd: BorrowedFd<'_>) {
        match self {
            Self::Ext(offer) => offer.receive(mime, fd),
            Self::Wlr(offer) => offer.receive(mime, fd),
        }
    }

    pub fn destroy(&self) {
        match self {
            Self::Ext(offer) => offer.destroy(),
            Self::Wlr(offer) => offer.destroy(),
        }
    }

    pub fn key(&self) -> wayland_client::backend::ObjectId {
        match self {
            Self::Ext(offer) => offer.id(),
            Self::Wlr(offer) => offer.id(),
        }
    }
}

impl Source {
    pub fn offer(&self, mime: &str) {
        match self {
            Self::Ext(source) => source.offer(mime.to_owned()),
            Self::Wlr(source) => source.offer(mime.to_owned()),
        }
    }

    pub fn destroy(&self) {
        match self {
            Self::Ext(source) => source.destroy(),
            Self::Wlr(source) => source.destroy(),
        }
    }
}
