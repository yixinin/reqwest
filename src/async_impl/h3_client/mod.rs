#[cfg(feature = "h3-quinn")]
pub mod quinn;

#[cfg(feature = "h3-s2n-quic")]
pub mod s2n_quic;

pub mod connection;
pub(crate) mod dns;
pub mod pool;
pub mod stream;

use self::pool::{Key, Pool, PoolClient};
use super::DynH3Connector;
use crate::async_impl::body::ResponseBody;
#[cfg(feature = "cookies")]
use crate::cookie;
use crate::error::{BoxError, Error, Kind};
use crate::{error, Body};
use http::{Request, Response};
use log::trace;
use std::future::{self, Future};
use std::pin::Pin;
#[cfg(feature = "cookies")]
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use sync_wrapper::SyncWrapper;
use tower::Service;

#[cfg(feature = "h3-quinn")]
pub use self::quinn::H3QuinnConnector;
#[cfg(feature = "h3-s2n-quic")]
pub use self::s2n_quic::H3S2nQuicConnector;
/// H3 Client Config
#[derive(Clone)]
pub(crate) struct H3ClientConfig {
    /// Set the maximum HTTP/3 header size this client is willing to accept.
    ///
    /// See [header size constraints] section of the specification for details.
    ///
    /// [header size constraints]: https://www.rfc-editor.org/rfc/rfc9114.html#name-header-size-constraints
    ///
    /// Please see docs in [`Builder`] in [`h3`].
    ///
    /// [`Builder`]: https://docs.rs/h3/latest/h3/client/struct.Builder.html#method.max_field_section_size
    pub(crate) max_field_section_size: Option<u64>,

    /// Enable whether to send HTTP/3 protocol grease on the connections.
    ///
    /// Just like in HTTP/2, HTTP/3 also uses the concept of "grease"
    ///
    /// to prevent potential interoperability issues in the future.
    /// In HTTP/3, the concept of grease is used to ensure that the protocol can evolve
    /// and accommodate future changes without breaking existing implementations.
    ///
    /// Please see docs in [`Builder`] in [`h3`].
    ///
    /// [`Builder`]: https://docs.rs/h3/latest/h3/client/struct.Builder.html#method.send_grease
    pub(crate) send_grease: Option<bool>,
}

impl Default for H3ClientConfig {
    fn default() -> Self {
        Self {
            max_field_section_size: None,
            send_grease: None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct H3Client {
    pool: Pool,
    connector: DynH3Connector,
    #[cfg(feature = "cookies")]
    cookie_store: Option<Arc<dyn cookie::CookieStore>>,
}

impl H3Client {
    #[cfg(not(feature = "cookies"))]
    pub fn new(connector: DynH3Connector, pool_timeout: Option<Duration>) -> Self {
        H3Client {
            pool: Pool::new(pool_timeout),
            connector,
        }
    }

    #[cfg(feature = "cookies")]
    pub fn new(
        connector: H3Connector,
        pool_timeout: Option<Duration>,
        cookie_store: Option<Arc<dyn cookie::CookieStore>>,
    ) -> Self {
        H3Client {
            pool: Pool::new(pool_timeout),
            connector,
            cookie_store,
        }
    }

    async fn get_pooled_client(&mut self, key: Key) -> Result<PoolClient, BoxError> {
        if let Some(client) = self.pool.try_pool(&key) {
            trace!("getting client from pool with key {key:?}");
            return Ok(client);
        }

        trace!("did not find connection {key:?} in pool so connecting...");

        let dest = pool::domain_as_uri(key.clone());

        let lock = match self.pool.connecting(&key) {
            pool::Connecting::InProgress(waiter) => {
                trace!("connecting to {key:?} is already in progress, subscribing...");

                match waiter.receive().await {
                    Some(client) => return Ok(client),
                    None => return Err("failed to establish connection for HTTP/3 request".into()),
                }
            }
            pool::Connecting::Acquired(lock) => lock,
        };
        trace!("connecting to {key:?}...");
        let (driver, tx) = self.connector.connect(dest).await?;
        trace!("saving new pooled connection to {key:?}");
        Ok(self.pool.new_connection(lock, driver, tx))
    }

    #[cfg(not(feature = "cookies"))]
    async fn send_request(
        mut self,
        key: Key,
        req: Request<Body>,
    ) -> Result<Response<ResponseBody>, Error> {
        let mut pooled = match self.get_pooled_client(key).await {
            Ok(client) => client,
            Err(e) => return Err(error::request(e)),
        };
        pooled
            .send_request(req)
            .await
            .map_err(|e| Error::new(Kind::Request, Some(e)))
    }

    #[cfg(feature = "cookies")]
    async fn send_request(
        mut self,
        key: Key,
        mut req: Request<Body>,
    ) -> Result<Response<ResponseBody>, Error> {
        let mut pooled = match self.get_pooled_client(key).await {
            Ok(client) => client,
            Err(e) => return Err(error::request(e)),
        };

        let url = url::Url::parse(req.uri().to_string().as_str()).unwrap();
        if let Some(cookie_store) = self.cookie_store.as_ref() {
            if req.headers().get(crate::header::COOKIE).is_none() {
                let headers = req.headers_mut();
                crate::util::add_cookie_header(headers, &**cookie_store, &url);
            }
        }

        let res = pooled
            .send_request(req)
            .await
            .map_err(|e| Error::new(Kind::Request, Some(e)));

        if let Some(ref cookie_store) = self.cookie_store {
            if let Ok(res) = &res {
                let mut cookies = cookie::extract_response_cookie_headers(res.headers()).peekable();
                if cookies.peek().is_some() {
                    cookie_store.set_cookies(&mut cookies, &url);
                }
            }
        }

        res
    }

    pub fn request(&self, mut req: Request<Body>) -> H3ResponseFuture {
        let pool_key = match pool::extract_domain(req.uri_mut()) {
            Ok(s) => s,
            Err(e) => {
                return H3ResponseFuture {
                    inner: SyncWrapper::new(Box::pin(future::ready(Err(e)))),
                }
            }
        };
        H3ResponseFuture {
            inner: SyncWrapper::new(Box::pin(self.clone().send_request(pool_key, req))),
        }
    }
}

impl Service<Request<Body>> for H3Client {
    type Response = Response<ResponseBody>;
    type Error = Error;
    type Future = H3ResponseFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.request(req)
    }
}

pub(crate) struct H3ResponseFuture {
    inner: SyncWrapper<Pin<Box<dyn Future<Output = Result<Response<ResponseBody>, Error>> + Send>>>,
}

impl Future for H3ResponseFuture {
    type Output = Result<Response<ResponseBody>, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.get_mut().as_mut().poll(cx)
    }
}
