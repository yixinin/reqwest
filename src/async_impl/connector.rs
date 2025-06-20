use std::sync::Arc;

use bytes::Bytes;
use h3::client::SendRequest;
use h3_quinn::{Connection, OpenStreams};
use http::Uri;
use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;

pub type H3Connection = (
    h3::client::Connection<Connection, Bytes>,
    SendRequest<OpenStreams, Bytes>,
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
