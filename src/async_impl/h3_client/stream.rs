use std::sync::Arc;

#[cfg(feature = "h3-quinn")]
use bytes::Buf;
use bytes::Bytes;
use h3::quic::OpenStreams;

#[derive(Clone)]
pub(crate) struct DynOpenStream<B>
where
    B: Buf,
{
    #[cfg(feature = "h3-quinn")]
    inner: Arc<
        dyn OpenStreams<
            B,
            BidiStream = h3_quinn::BidiStream<B>,
            SendStream = h3_quinn::SendStream<B>,
        >,
    >,
    #[cfg(feature = "h3-s2n-quic")]
    inner: Arc<
        dyn OpenStreams<
            B,
            BidiStream = super::s2n_quic::BidiStream<B>,
            SendStream = super::s2n_quic::SendStream<B>,
        >,
    >,
}

impl<B: Buf> DynOpenStream<B> {
    #[cfg(feature = "h3-s2n-quic")]
    pub fn new(inner: Arc<super::s2n_quic::OpenStreams>) -> Self {
        Self { inner }
    }
    #[cfg(feature = "h3-quinn")]
    pub fn new(inner: Arc<h3_quinn::OpenStreams>) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}

#[cfg(feature = "h3-quinn")]
impl<B: Buf> OpenStreams<Bytes> for DynOpenStream<B> {
    type BidiStream = h3_quinn::BidiStream<B>;

    type SendStream = h3_quinn::SendStream<B>;

    fn poll_open_bidi(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::BidiStream, h3::quic::StreamErrorIncoming>> {
        self.inner.clone().poll_open_bidi(cx)
    }

    fn poll_open_send(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Self::SendStream, h3::quic::StreamErrorIncoming>> {
        self.inner.poll_open_send(cx)
    }

    fn close(&mut self, code: h3::error::Code, reason: &[u8]) {
        self.inner.close(code, reason);
    }
}
