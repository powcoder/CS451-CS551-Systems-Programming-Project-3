https://powcoder.com
代写代考加微信 powcoder
Assignment Project Exam Help
Add WeChat powcoder
use super::Command;
use crate::{connection::ConnectionData, parser::Parser, Bing2BingError, Bing2BingFrame};
use async_trait::async_trait;
use bytes::Bytes;
use std::convert::TryFrom;

/// `Deliver` data [Bing2BingFrame::Bulk] to a specific destination (peer).
/// # Points available
/// The current implementation of [`Deliver::apply()`] forwards to all connected peers.
/// To receive additional points, you should make an attempt to determine the shortest path
/// and forward _only_ to the next hop in that path.
#[derive(Debug, Clone)]
pub struct Deliver {
    pub(crate) source: String,
    pub(crate) sequence_number: u64,
    destination: String,
    data: Bytes,
}

impl Deliver {
    /// Turns this `Deliver` into a [Bing2BingFrame].
    pub fn into_frame(self) -> Bing2BingFrame {
        // note that using the vec! macro like this is more
        // performant than creating a new vector and then
        // pushing into it according to clippy:
        // https://rust-lang.github.io/rust-clippy/master/index.html#vec_init_then_push
        let cmd = vec![
            Bing2BingFrame::Text("deliver".to_string()),
            Bing2BingFrame::Text(self.source),
            Bing2BingFrame::Number(self.sequence_number),
            Bing2BingFrame::Text(self.destination),
            Bing2BingFrame::Bulk(self.data.to_vec()),
        ];

        // cmd.push(Bing2BingFrame::Text("deliver".to_string()));
        // cmd.push(Bing2BingFrame::Text(self.source));
        // cmd.push(Bing2BingFrame::Number(self.sequence_number));
        // cmd.push(Bing2BingFrame::Text(self.destination));
        // cmd.push(Bing2BingFrame::Bulk(self.data.to_vec()));

        Bing2BingFrame::Array(cmd)
    }
}

#[async_trait]
impl Command for Deliver {
    fn get_sequence_number(&self) -> u64 {
        self.sequence_number
    }

    fn get_source(&self) -> String {
        self.source.clone()
    }

    /// Forward the data on to the next peer in the path.
    /// # Points available
    /// Current implementation just broadcasts the command out to all connected peers.
    async fn apply(self, connection_data: &mut ConnectionData) -> Result<(), Bing2BingError> {
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

        let data = parse.next_bytes()?;

        parse.finish()?;

        Ok(Self {
            source,
            sequence_number,
            destination,
            data,
        })
    }
}

impl From<Deliver> for Bing2BingFrame {
    fn from(value: Deliver) -> Self {
        value.into_frame()
    }
}

impl TryFrom<&mut Parser> for Deliver {
    type Error = Bing2BingError;

    fn try_from(value: &mut Parser) -> Result<Self, Self::Error> {
        Self::parse_frames(value)
    }
}
