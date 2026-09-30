//! `org.freedesktop.Application` (DEF-04): how launchers hand Wye links when
//! the desktop entry says `DBusActivatable=true`.

use wye_api::Error;
use zbus::message::Header;
use zbus::zvariant::OwnedValue;

use crate::api::{self, Caller, Dict};
use crate::context::ServiceContext;

/// Desktop activation.
#[derive(Debug, Clone)]
pub struct Application {
    ctx: ServiceContext,
}

impl Application {
    /// The interface over `ctx`.
    #[must_use]
    pub fn new(ctx: ServiceContext) -> Self {
        Self { ctx }
    }
}

#[zbus::interface(name = "org.freedesktop.Application")]
impl Application {
    async fn activate(
        &self,
        #[zbus(header)] header: Header<'_>,
        platform_data: Dict,
    ) -> Result<(), Error> {
        api::windows::activate(&self.ctx, &Caller::from_header(&header), &platform_data).await
    }

    async fn open(
        &self,
        #[zbus(header)] header: Header<'_>,
        uris: Vec<String>,
        platform_data: Dict,
    ) -> Result<(), Error> {
        api::link::open_uris(
            &self.ctx,
            &Caller::from_header(&header),
            &uris,
            &platform_data,
        )
        .await
    }

    async fn activate_action(
        &self,
        #[zbus(header)] header: Header<'_>,
        action_name: &str,
        parameter: Vec<OwnedValue>,
        platform_data: Dict,
    ) -> Result<(), Error> {
        let caller = Caller::from_header(&header);
        api::windows::activate_action(&self.ctx, &caller, action_name, &parameter, &platform_data)
            .await
    }
}
