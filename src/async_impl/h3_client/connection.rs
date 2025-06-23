use std::sync::Arc;

use h3::quic::{Connection, OpenStreams, RecvStream};

use bytes::{Buf, Bytes};

use crate::async_impl::h3_client::stream::DynOpenStream;

#[derive(Clone)]
pub(crate) struct DynConnection<B>
where
    B: Buf,
{
    #[cfg(feature = "h3-s2n-quic")]
    inner: Arc<
        dyn Connection<B, RecvStream = super::s2n_quic::RecvStream, OpenStreams = DynOpenStream<B>>,
    >,
    #[cfg(feature = "h3-quinn")]
    inner:
        Arc<dyn Connection<B, RecvStream = h3_quinn::RecvStream, OpenStreams = DynOpenStream<B>>>,
}

impl<B: Buf> DynConnection<B> {
    #[cfg(feature = "h3-s2n-quic")]
    pub fn new(inner: Arc<super::s2n_quic::Connection>) -> Self {
        Self { inner }
    }
    #[cfg(feature = "h3-quinn")]
    pub fn new(inner: Arc<h3_quinn::Connection>) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "h3-s2n-quic")]
impl<B: Buf> h3::quic::Connection<B> for DynConnection<B> {
    type RecvStream = super::s2n_quic::RecvStream;

    type OpenStreams = super::s2n_quic::OpenStreams;

    fn poll_accept_recv(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::RecvStream, h3::quic::ConnectionErrorIncoming>> {
        self.inner.clone().poll_accept_recv(cx)
    }

    fn poll_accept_bidi(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::BidiStream, h3::quic::ConnectionErrorIncoming>> {
        self.inner.clone().poll_accept_bidi(cx)
    }

    fn opener(&self) -> Self::OpenStreams {
        self.inner.clone().opener()
    }
}

#[cfg(feature = "h3-quinn")]
impl<B: Buf> h3::quic::Connection<B> for DynConnection<B> {
    type RecvStream = h3_quinn::RecvStream;

    type OpenStreams = h3_quinn::OpenStreams;

    fn poll_accept_recv(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::RecvStream, h3::quic::ConnectionErrorIncoming>> {
        self.inner.clone().poll_accept_recv(cx)
    }

    fn poll_accept_bidi(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::BidiStream, h3::quic::ConnectionErrorIncoming>> {
        self.inner.clone().poll_accept_bidi(cx)
    }

    fn opener(&self) -> Self::OpenStreams {
        self.inner.clone().opener()
    }
}
