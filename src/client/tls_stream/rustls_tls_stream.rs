use crate::{
    client::{config::Config, TrustConfig},
    error::IoErrorKind,
    observability, Error,
};
use futures_util::io::{AsyncRead, AsyncWrite};
use std::{
    fmt, fs, io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio_rustls::{
    rustls::{
        client::{
            danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
            WantsClientCert,
        },
        crypto::ring,
        pki_types::{CertificateDer, ServerName, UnixTime},
        ClientConfig, ConfigBuilder, DigitallySignedStruct, Error as RustlsError, RootCertStore,
        SignatureScheme, WantsVerifier,
    },
    TlsConnector,
};
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};

impl From<tokio_rustls::rustls::Error> for Error {
    fn from(e: tokio_rustls::rustls::Error) -> Self {
        crate::Error::Tls(e.to_string())
    }
}

pub(crate) struct TlsStream<S: AsyncRead + AsyncWrite + Unpin + Send>(
    Compat<tokio_rustls::client::TlsStream<Compat<S>>>,
);

struct NoCertVerifier;

impl fmt::Debug for NoCertVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NoCertVerifier").finish()
    }
}

impl ServerCertVerifier for NoCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn get_server_name(config: &Config) -> crate::Result<ServerName<'static>> {
    match (
        ServerName::try_from(config.get_host()).map(|name| name.to_owned()),
        &config.trust,
    ) {
        (Ok(sn), _) => Ok(sn),
        (Err(_), TrustConfig::TrustAll) => {
            Ok(ServerName::try_from("placeholder.domain.com").unwrap())
        }
        (Err(e), _) => Err(crate::Error::Tls(e.to_string())),
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> TlsStream<S> {
    pub(super) async fn new(config: &Config, stream: S) -> crate::Result<Self> {
        let builder = ClientConfig::builder();

        let client_config = match &config.trust {
            TrustConfig::CaCertificateLocation(path) => {
                observability::tls::emit_trust_config(
                    observability::tls::backend_name(),
                    "ca_certificate",
                    true,
                );

                if let Ok(buf) = fs::read(path) {
                    let cert = match path.extension() {
                            Some(ext)
                            if ext.eq_ignore_ascii_case("pem")
                                || ext.eq_ignore_ascii_case("crt") =>
                                {
                                    use tokio_rustls::rustls::pki_types::pem::PemObject;

                                    let pem_cert = CertificateDer::pem_slice_iter(&buf)
                                        .collect::<Result<Vec<_>, _>>()
                                        .map_err(|err| crate::Error::Io {
                                            kind: IoErrorKind::InvalidInput,
                                            message: format!(
                                                "Could not parse provided CA certificate: {}",
                                                err
                                            ),
                                        })?;
                                    if pem_cert.len() != 1 {
                                        return Err(crate::Error::Io {
                                            kind: IoErrorKind::InvalidInput,
                                            message: format!("Certificate file {} contain 0 or more than 1 certs", path.to_string_lossy()),
                                        });
                                    }

                                    pem_cert.into_iter().next().unwrap()
                                }
                            Some(ext) if ext.eq_ignore_ascii_case("der") => {
                                CertificateDer::from(buf)
                            }
                            Some(_) | None => return Err(crate::Error::Io {
                                kind: IoErrorKind::InvalidInput,
                                message: "Provided CA certificate with unsupported file-extension! Supported types are pem, crt and der.".to_string(),
                            }),
                        };
                    let mut cert_store = RootCertStore::empty();
                    cert_store.add(cert)?;
                    builder
                        .with_root_certificates(cert_store)
                        .with_no_client_auth()
                } else {
                    return Err(Error::Io {
                        kind: IoErrorKind::InvalidData,
                        message: "Could not read provided CA certificate!".to_string(),
                    });
                }
            }
            TrustConfig::TrustAll => {
                observability::tls::emit_trust_config(
                    observability::tls::backend_name(),
                    "trust_all",
                    false,
                );

                builder
                    .dangerous()
                    .with_custom_certificate_verifier(Arc::new(NoCertVerifier {}))
                    .with_no_client_auth()
            }
            TrustConfig::Default => {
                observability::tls::emit_trust_config(
                    observability::tls::backend_name(),
                    "default",
                    true,
                );
                builder.with_native_roots()?.with_no_client_auth()
            }
        };

        let connector = TlsConnector::from(Arc::new(client_config));

        let tls_stream = connector
            .connect(get_server_name(config)?, stream.compat())
            .await?;

        Ok(TlsStream(tls_stream.compat()))
    }

    pub(crate) fn get_mut(&mut self) -> &mut S {
        self.0.get_mut().get_mut().0.get_mut()
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncRead for TlsStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let inner = Pin::get_mut(self);
        Pin::new(&mut inner.0).poll_read(cx, buf)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> AsyncWrite for TlsStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let inner = Pin::get_mut(self);
        Pin::new(&mut inner.0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let inner = Pin::get_mut(self);
        Pin::new(&mut inner.0).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let inner = Pin::get_mut(self);
        Pin::new(&mut inner.0).poll_close(cx)
    }
}

trait ConfigBuilderExt {
    fn with_native_roots(self) -> crate::Result<ConfigBuilder<ClientConfig, WantsClientCert>>;
}

impl ConfigBuilderExt for ConfigBuilder<ClientConfig, WantsVerifier> {
    fn with_native_roots(self) -> crate::Result<ConfigBuilder<ClientConfig, WantsClientCert>> {
        let mut roots = RootCertStore::empty();
        let mut valid_count = 0;
        let mut invalid_count = 0;

        let cert_result = rustls_native_certs::load_native_certs();
        invalid_count += u64::try_from(cert_result.errors.len()).unwrap_or(u64::MAX);

        for cert in cert_result.certs {
            match roots.add(cert.clone()) {
                Ok(_) => valid_count += 1,
                Err(_) => invalid_count += 1,
            }
        }

        observability::tls::emit_root_certificates_loaded(
            observability::tls::backend_name(),
            valid_count,
            invalid_count,
        );
        assert!(!roots.is_empty(), "no CA certificates found");

        Ok(self.with_root_certificates(roots))
    }
}
