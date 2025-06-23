use std::sync::Arc;

use bytes::Bytes;

#[cfg(feature = "http3")]
use h3::client::SendRequest;

use http::Uri;

// #[cfg(feature = "h3-quinn")]
// use h3_quinn::{Connection, OpenStreams};
// use http::Uri;

// #[cfg(feature = "h3-s2n-quic")]
// use super::s2n_quic_h3::{Connection, OpenStreams};

use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;

use crate::async_impl::h3_client::connection::DynConnection;
use crate::async_impl::h3_client::stream::DynOpenStream;

pub type H3Connection = (
    h3::client::Connection<DynConnection<Bytes>>,
    SendRequest<DynOpenStream<Bytes>, Bytes>,
);

///
pub type H3Connecting =
    Pin<Box<dyn Future<Output = Result<H3Connection, Box<dyn StdError + Send + Sync>>> + Send>>;

///
pub trait H3Connector: std::fmt::Debug + Send + Sync {
    ///
    fn connect(&self, dest: Uri) -> H3Connecting;
}
///
#[derive(Debug, Clone)]
pub struct DynH3Connector {
    connector: Arc<dyn H3Connector>,
}

///
impl DynH3Connector {
    ///
    pub fn new(connector: Arc<dyn H3Connector>) -> Self {
        return Self { connector };
    }
    ///
    pub fn connect(&self, dest: Uri) -> H3Connecting {
        self.connector.connect(dest)
    }
}
