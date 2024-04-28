https://powcoder.com
代写代考加微信 powcoder
Assignment Project Exam Help
Add WeChat powcoder
use super::Command;
use crate::{connection::ConnectionData, parser::Parser, Bing2BingError, Bing2BingFrame};
use async_trait::async_trait;
use std::convert::TryFrom;
use tracing::{instrument, trace};

/// This command allows for direct messaging between two peers.
/// The idea is that peers should forward this message via the shortest path to the target.
///
/// # Points available.
///
/// Currently, [Whisper::apply()] just treats things as a [Say](crate::cmd::Say), and thus
/// the message is broadcast to all outgoing peers.
/// For extra points, make it only send the data out over the next hop in the shortest path
/// to the destination.
#[derive(Debug, Clone)]
pub struct Whisper {
    pub(crate) source: String,
    pub(crate) sequence_number: u64,
    pub(crate) destination: String,
    pub(crate) message: String,
}

impl Whisper {
    pub fn new(source: &str, sequence_number: u64, destination: &str, message: &str) -> Self {
        let source = source.to_string();
        let destination = destination.to_string();
        let message = message.to_string();

        Self {
            source,
            sequence_number,
            destination,
            message,
        }
    }

    pub fn into_frame(self) -> Bing2BingFrame {
        let cmd = vec![
            Bing2BingFrame::Text("whisper".to_string()),
            Bing2BingFrame::Text(self.source),
            Bing2BingFrame::Number(self.sequence_number),
            Bing2BingFrame::Text(self.destination),
            Bing2BingFrame::Text(self.message),
        ];

        Bing2BingFrame::Array(cmd)
    }
}

#[async_trait]
impl Command for Whisper {
    fn get_sequence_number(&self) -> u64 {
        self.sequence_number
    }

    fn get_source(&self) -> String {
        self.source.clone()
    }

    /// Currently just broadcasts the message back out to everyone else
    /// This will (eventually) mean that the whisper will arrive at its
    /// destination.
    #[instrument(level = "trace")]
    async fn apply(self, connection_data: &mut ConnectionData) -> Result<(), Bing2BingError> {
        trace!("Applying Whisper command: {:?}", self);

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
        let destination = parse.next_text()?;

        let message = parse.next_text()?;

        parse.finish()?;

        Ok(Self::new(&source, sequence_number, &destination, &message))
    }
}

impl From<Whisper> for Bing2BingFrame {
    fn from(value: Whisper) -> Self {
        value.into_frame()
    }
}

impl TryFrom<&mut Parser> for Whisper {
    type Error = Bing2BingError;

    fn try_from(value: &mut Parser) -> Result<Self, Self::Error> {
        Self::parse_frames(value)
    }
}
