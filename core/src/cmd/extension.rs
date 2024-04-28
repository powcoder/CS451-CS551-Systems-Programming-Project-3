https://powcoder.com
代写代考加微信 powcoder
Assignment Project Exam Help
Add WeChat powcoder
use super::Command;
use crate::{
    connection::ConnectionData, parser::Parser, peer_map::PeerMap, Bing2BingError, Bing2BingFrame,
};
use async_trait::async_trait;
use std::convert::TryFrom;
use tracing::{instrument, trace, warn};

/// This command serves as a mechanism to enable extensions to the protocol.
/// It is esssentially a wrapper around:
///
/// 1. An extension id, which uniquely represents the given extension.
/// 2. A payload [Bing2BingFrame] that is used for whatever the extension is
/// is supposed to do.
///
/// # Points available
///
/// Develop an `Extension`!
#[derive(Debug, Clone)]
pub struct Extension {
    pub(crate) source: String,
    pub(crate) sequence_number: u64,
    pub(crate) extension_id: u64,
    pub(crate) payload: Bing2BingFrame,
}

impl Extension {
    pub fn new(
        source: String,
        sequence_number: u64,
        extension_id: u64,
        payload: Bing2BingFrame,
    ) -> Self {
        Self {
            source,
            sequence_number,
            extension_id,
            payload,
        }
    }

    /// Currently just broadcasts the message back out to everyone else
    /// This will (eventually) mean that the `Extension` will arrive at its
    /// destination.
    #[instrument(level = "trace")]
    pub(crate) async fn my_apply(&self, peer_map: &PeerMap) -> Result<(), Bing2BingError> {
        trace!("Applying Extension command: {:?}", self);

        let frame = self.clone().into_frame();

        warn!(
            "Unimplemented command ({:?}); broadcasting for propagation",
            self
        );
        peer_map.broadcast(&self.source, frame);

        Ok(())
    }

    /// Turns this `Extension` into a [Bing2BingFrame].
    pub fn into_frame(self) -> Bing2BingFrame {
        let cmd = vec![
            Bing2BingFrame::Text("extension".to_string()),
            Bing2BingFrame::Text(self.source),
            Bing2BingFrame::Number(self.sequence_number),
            Bing2BingFrame::Number(self.extension_id),
            self.payload,
        ];

        Bing2BingFrame::Array(cmd)
    }
}

#[async_trait]
impl Command for Extension {
    fn get_sequence_number(&self) -> u64 {
        self.sequence_number
    }

    fn get_source(&self) -> String {
        self.source.clone()
    }
    /// Currently just broadcasts the message back out to everyone else
    /// This will (eventually) mean that the `Extension` will arrive at its
    /// destination.
    #[instrument(level = "trace")]
    async fn apply(self, connection_data: &mut ConnectionData) -> Result<(), Bing2BingError> {
        trace!("Applying Extension command: {:?}", self);

        // let frame = self.clone().into_frame();

        warn!(
            "Unimplemented command ({:?}); broadcasting for propagation",
            self
        );
        connection_data
            .peers
            .broadcast(&self.source.clone(), self.into());

        Ok(())
    }

    fn parse_frames(parse: &mut Parser) -> Result<Self, Bing2BingError>
    where
        Self: Sized,
    {
        let source = parse.next_text()?;

        let sequence_number = parse.next_number()?;
        let extension_id = parse.next_number()?;

        let payload = parse.next()?;

        parse.finish()?;

        Ok(Self::new(source, sequence_number, extension_id, payload))
    }
}

impl From<Extension> for Bing2BingFrame {
    fn from(value: Extension) -> Self {
        value.into_frame()
    }
}

impl TryFrom<&mut Parser> for Extension {
    type Error = Bing2BingError;

    fn try_from(value: &mut Parser) -> Result<Self, Self::Error> {
        Self::parse_frames(value)
    }
}
