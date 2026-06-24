use super::SqlBrowser;
use crate::{client::Config, observability};
use async_io::Timer;
use async_net::{resolve, TcpStream, UdpSocket};
use async_trait::async_trait;
use futures_lite::FutureExt;
use futures_util::future::TryFutureExt;
use std::io;
use std::time::Duration;

#[async_trait]
impl SqlBrowser for TcpStream {
    /// This method can be used to connect to SQL Server named instances
    /// when on a Windows paltform with the `sql-browser-tokio` feature
    /// enabled. Please see the crate examples for more detailed examples.
    async fn connect_named(builder: &Config) -> crate::Result<Self> {
        let addrs = resolve(builder.get_addr()).await?;

        for mut addr in addrs {
            if let Some(ref instance_name) = builder.instance_name {
                // First resolve the instance to a port via the
                // SSRP protocol/MS-SQLR protocol [1]
                // [1] https://msdn.microsoft.com/en-us/library/cc219703.aspx

                let local_bind: std::net::SocketAddr = if addr.is_ipv4() {
                    "0.0.0.0:0".parse().unwrap()
                } else {
                    "[::]:0".parse().unwrap()
                };
                let address_family = if addr.is_ipv4() { "ipv4" } else { "ipv6" };
                let sql_browser_port = builder.get_port();

                observability::sql_browser::emit_resolution_start(
                    "smol",
                    address_family,
                    sql_browser_port,
                );

                let msg = [&[4u8], instance_name.as_bytes()].concat();
                let mut buf = vec![0u8; 4096];

                let socket = UdpSocket::bind(&local_bind).await?;
                socket.send_to(&msg, &addr).await?;

                let timeout = Duration::from_millis(1000);

                let len = socket
                    .recv(&mut buf)
                    .or(async {
                        Timer::after(timeout).await;
                        Err(std::io::ErrorKind::TimedOut.into())
                    })
                    .map_err(|e| {
                        if e.kind() == std::io::ErrorKind::TimedOut {
                            observability::sql_browser::emit_resolution_timeout(
                                "smol",
                                address_family,
                                sql_browser_port,
                                timeout,
                            );

                            crate::error::Error::Conversion(
                                format!(
                                    "SQL browser timeout during resolving instance {}. Please check if browser is running in port {} and does the instance exist.",
                                    instance_name,
                                    sql_browser_port,
                                )
                                .into(),
                            )
                        } else {
                            e.into()
                        }
                    })
                    .await?;

                let port = match super::get_port_from_sql_browser_reply(buf, len, instance_name) {
                    Ok(port) => {
                        observability::sql_browser::emit_resolution_completed(
                            "smol",
                            address_family,
                            sql_browser_port,
                            port,
                        );
                        port
                    }
                    Err(error) => {
                        observability::sql_browser::emit_resolution_failed(
                            "smol",
                            address_family,
                            sql_browser_port,
                            &error,
                        );
                        return Err(error);
                    }
                };
                addr.set_port(port);
            };

            if let Ok(stream) = TcpStream::connect(addr).await {
                stream.set_nodelay(true)?;
                return Ok(stream);
            }
        }

        Err(io::Error::new(io::ErrorKind::NotFound, "Could not resolve server host").into())
    }
}
