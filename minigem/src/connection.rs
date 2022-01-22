use crate::{Error, Request, Response};
use rustls::{ClientConnection, OwnedTrustAnchor, RootCertStore, StreamOwned};
use std::io::{self, Write};
use std::net::TcpStream;
use std::sync::Arc;
use url::Url;

const GEMINI_DEFAULT_PORT: u16 = 1965;

pub(crate) type GeminiStream = StreamOwned<ClientConnection, TcpStream>;

pub(crate) struct Connection {
    request: Request,
}

impl Connection {
    pub(crate) fn new(request: Request) -> Self {
        Self { request }
    }

    pub(crate) fn send(&mut self) -> Result<Response<GeminiStream>, Error> {
        let url = Url::parse(self.request.url.as_ref())?;

        // URL is parsed before send() is called; therefore, we can just unwrap.
        let host = url.host_str().unwrap();
        let port = url.port().unwrap_or(GEMINI_DEFAULT_PORT);

        let mut stream = connect(host, port)?;
        write!(stream, "{}\r\n", url.as_str())?;

        Response::try_from_reader(stream)
    }
}

fn connect(host: &str, port: u16) -> Result<GeminiStream, io::Error> {
    let mut root_store = RootCertStore::empty();
    root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));

    let mut config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    config
        .dangerous()
        .set_certificate_verifier(Arc::new(NoCertificateVerification));

    let server_name = host
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid DNS name"))?;
    let conn = ClientConnection::new(Arc::new(config), server_name)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    let stream = TcpStream::connect((host, port))?;

    Ok(StreamOwned::new(conn, stream))
}

struct NoCertificateVerification;

impl rustls::client::ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
