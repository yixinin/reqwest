use crate::async_impl::connector::{H3Connection, H3Connector};
use crate::async_impl::h3_client::dns::resolve;
use crate::async_impl::H3Connecting;
use crate::dns::DynResolver;
use crate::error::BoxError;

use super::{Connection, OpenStreams};
use crate::async_impl::h3_client::H3ClientConfig;
use http::Uri;
use hyper_util::client::legacy::connect::dns::Name;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone)]
pub struct H3S2nQuicConnector {
    resolver: DynResolver,
    client: s2n_quic::Client,
    client_config: H3ClientConfig,
}

impl H3S2nQuicConnector {
    pub fn new(
        resolver: DynResolver,
        tls: rustls::ClientConfig,
        local_addr: Option<IpAddr>,
        local_port: Option<u16>,
        transport_config: TransportConfig,
        client_config: H3ClientConfig,
    ) -> Result<H3S2nQuicConnector, BoxError> {
        let socket_addr = match local_addr {
            Some(ip) => SocketAddr::new(ip, local_port.unwrap_or(0)),
            None => format!("[::]:{}", local_port.unwrap_or(0))
                .parse::<SocketAddr>()
                .unwrap(),
        };

        // let mut endpoint = Endpoint::client(socket_addr)?;
        // endpoint.set_default_client_config(config);
        let client = s2n_quic::Client::builder()
            .with_io(socket_addr)?
            .with_tls(tls)?
            .start()?;

        Ok(Self {
            resolver,
            client,
            client_config,
        })
    }

    pub async fn connect_dest(&mut self, dest: Uri) -> Result<H3Connection, BoxError> {
        let host = dest
            .host()
            .ok_or("destination must have a host")?
            .trim_start_matches('[')
            .trim_end_matches(']');
        let port = dest.port_u16().unwrap_or(443);

        let addrs = if let Some(addr) = IpAddr::from_str(host).ok() {
            // If the host is already an IP address, skip resolving.
            vec![SocketAddr::new(addr, port)]
        } else {
            let addrs = resolve(&mut self.resolver, Name::from_str(host)?).await?;
            let addrs = addrs.map(|mut addr| {
                addr.set_port(port);
                addr
            });
            addrs.collect()
        };

        self.remote_connect(addrs, host).await
    }

    async fn remote_connect(
        &mut self,
        addrs: Vec<SocketAddr>,
        server_name: &str,
    ) -> Result<H3Connection, BoxError> {
        let mut err = None;
        for addr in addrs {
            match self.endpoint.connect(addr, server_name)?.await {
                Ok(new_conn) => {
                    let quinn_conn = Connection::new(new_conn);
                    let mut h3_client_builder = h3::client::builder();
                    if let Some(max_field_section_size) = self.client_config.max_field_section_size
                    {
                        h3_client_builder.max_field_section_size(max_field_section_size);
                    }
                    if let Some(send_grease) = self.client_config.send_grease {
                        h3_client_builder.send_grease(send_grease);
                    }
                    return Ok(h3_client_builder.build(quinn_conn).await?);
                }
                Err(e) => err = Some(e),
            }
        }

        match err {
            Some(e) => Err(Box::new(e) as BoxError),
            None => Err("failed to establish connection for HTTP/3 request".into()),
        }
    }
}

impl H3Connector for H3S2nQuicConnector {
    fn connect(&self, dest: Uri) -> H3Connecting {
        let mut connector = self.clone();
        Box::pin(async move {
            let connection = connector.connect_dest(dest).await?;
            Ok(connection)
        })
    }
}

impl std::fmt::Debug for H3S2nQuicConnector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "quinn quic connector")
    }
}
