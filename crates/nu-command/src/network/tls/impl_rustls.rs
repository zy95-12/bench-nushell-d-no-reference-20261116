use std::sync::{Arc, LazyLock, OnceLock};

#[cfg(feature = "os")]
use nu_engine::command_prelude::IoError;
use nu_protocol::ShellError;
use rustls::crypto::CryptoProvider;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

// TODO: replace all these generic errors with proper errors

/// Stores the crypto provider used by `rustls`.
///
/// This struct lives in the [`CRYPTO_PROVIDER`] static.  
/// It can't be created manually.
///
/// ## Purpose
///
/// Nushell does **not** use the global `rustls` crypto provider.  
/// You **must** set a provider here—otherwise, any networking command
/// that uses `rustls` won't be able to build a TLS connector.
///
/// This only matters if the **`rustls-tls`** feature is enabled.  
/// Builds with **`native-tls`** ignore this completely.
///
/// ## How to set the provider
///
/// * [`NuCryptoProvider::default`]  
///   Uses a built-in provider that works with official `nu` builds.  
///   This might change in future versions.
///
/// * [`NuCryptoProvider::set`]  
///   Lets you provide your own `CryptoProvider` using a closure:
///
///   ```rust
///   use nu_command::tls::CRYPTO_PROVIDER;
///
///   // Call once at startup
///   CRYPTO_PROVIDER.set(|| Ok(rustls::crypto::ring::default_provider()));
///   ```
///
/// Only the first successful call takes effect. Later calls do nothing and return `false`.
#[derive(Debug)]
pub struct NuCryptoProvider(OnceLock<Result<Arc<CryptoProvider>, ShellError>>);

/// Global [`NuCryptoProvider`] instance.
///
/// When the **`rustls-tls`** feature is active, call
/// [`CRYPTO_PROVIDER.default()`](NuCryptoProvider::default) or  
/// [`CRYPTO_PROVIDER.set(...)`](NuCryptoProvider::set) once at startup
/// to pick the [`CryptoProvider`] that [`rustls`] will use.
///
/// Later TLS code gets the provider using [`get`](NuCryptoProvider::get).  
/// If no provider was set or the closure returned an error, `get` returns a [`ShellError`].
pub static CRYPTO_PROVIDER: NuCryptoProvider = NuCryptoProvider(OnceLock::new());

impl NuCryptoProvider {
    /// Returns the current [`CryptoProvider`].
    ///
    /// Comes from the first call to [`default`](Self::default) or [`set`](Self::set).
    ///
    /// # Errors
    /// - If no provider was set.
    /// - If the `set` closure returned an error.
    pub fn get(&self) -> Result<Arc<CryptoProvider>, ShellError> {
        // we clone here as the Arc for Ok is super cheap and basically all APIs expect an owned
        // ShellError, so we might as well clone here already
        match self.0.get() {
            Some(val) => val.clone(),
            None => Err(ShellError::GenericError {
                error: "tls crypto provider not found".to_string(),
                msg: "no crypto provider for rustls was defined".to_string(),
                span: None,
                help: Some("ensure that nu_command::tls::CRYPTO_PROVIDER is set".to_string()),
                inner: vec![],
            }),
        }
    }

    /// Sets a custom [`CryptoProvider`].
    ///
    /// Call once at startup, before any TLS code runs.  
    /// The closure runs immediately and the result (either `Ok` or `Err`) is stored.  
    /// Returns whether the provider was stored successfully.
    pub fn set(&self, f: impl FnOnce() -> Result<CryptoProvider, ShellError>) -> bool {
        let value = f().map(Arc::new);
        self.0.set(value).is_ok()
    }

    /// Sets a default [`CryptoProvider`] used in official `nu` builds.
    ///
    /// Should work on most systems, but may not work in every setup.  
    /// If it fails, use [`set`](Self::set) to install a custom one.  
    /// Returns whether the provider was stored successfully.
    pub fn default(&self) -> bool {
        self.set(|| Ok(rustls::crypto::ring::default_provider()))
    }
}

#[cfg(feature = "os")]
static ROOT_CERT_STORE: LazyLock<Result<RootCerts, ShellError>> = LazyLock::new(|| {
    let mut roots = Vec::new();

    let native_certs = rustls_native_certs::load_native_certs();

    let errors: Vec<_> = native_certs
        .errors
        .into_iter()
        .map(|err| match err.kind {
            rustls_native_certs::ErrorKind::Io { inner, path } => ShellError::Io(
                IoError::new_internal_with_path(inner, err.context, nu_protocol::location!(), path),
            ),
            rustls_native_certs::ErrorKind::Os(error) => ShellError::GenericError {
                error: error.to_string(),
                msg: err.context.to_string(),
                span: None,
                help: None,
                inner: vec![],
            },
            rustls_native_certs::ErrorKind::Pem(error) => ShellError::GenericError {
                error: error.to_string(),
                msg: err.context.to_string(),
                span: None,
                help: None,
                inner: vec![],
            },
            _ => ShellError::GenericError {
                error: String::from("unknown error loading native certs"),
                msg: err.context.to_string(),
                span: None,
                help: None,
                inner: vec![],
            },
        })
        .collect();
    if !errors.is_empty() {
        return Err(ShellError::GenericError {
            error: String::from("error loading native certs"),
            msg: String::from("could not load native certs"),
            span: None,
            help: None,
            inner: errors,
        });
    }

    // Validate every native certificate as before instead of silently ignoring malformed roots.
    let mut store = rustls::RootCertStore::empty();
    for cert in native_certs.certs {
        store
            .add(cert.clone())
            .map_err(|err| ShellError::GenericError {
                error: err.to_string(),
                msg: String::from("could not add root cert"),
                span: None,
                help: None,
                inner: vec![],
            })?;
        roots.push(ureq::tls::Certificate::from_der(cert.as_ref()).to_owned());
    }

    Ok(roots.into())
});

#[cfg(not(feature = "os"))]
static ROOT_CERT_STORE: LazyLock<Result<RootCerts, ShellError>> =
    LazyLock::new(|| Ok(RootCerts::WebPki));

#[doc = include_str!("./tls.rustdoc.md")]
pub fn tls(allow_insecure: bool) -> Result<TlsConfig, ShellError> {
    Ok(TlsConfig::builder()
        .provider(TlsProvider::Rustls)
        .unversioned_rustls_crypto_provider(CRYPTO_PROVIDER.get()?)
        .root_certs(if allow_insecure {
            RootCerts::WebPki
        } else {
            ROOT_CERT_STORE.clone()?
        })
        .disable_verification(allow_insecure)
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    // Self-signed localhost certificate generated for these tests only; never a trusted root.
    #[test]
    fn self_signed_certificate_requires_insecure_mode() {
        CRYPTO_PROVIDER.default();
        for insecure in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let certificate = rustls::pki_types::CertificateDer::from(
                    include_bytes!("test_assets/localhost.der").to_vec(),
                );
                let key = rustls::pki_types::PrivatePkcs8KeyDer::from(
                    include_bytes!("test_assets/localhost-key.der").to_vec(),
                );
                let config =
                    rustls::ServerConfig::builder_with_provider(CRYPTO_PROVIDER.get().unwrap())
                        .with_safe_default_protocol_versions()
                        .unwrap()
                        .with_no_client_auth()
                        .with_single_cert(vec![certificate], key.into())
                        .unwrap();
                let (socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
                let mut stream = rustls::StreamOwned::new(connection, socket);
                let mut buf = [0; 4096];
                // A failed handshake is expected for the verified request.
                if stream.read(&mut buf).is_ok() {
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\ntrusted",
                    );
                    let _ = stream.flush();
                }
            });
            let result = ureq::Agent::config_builder()
                .tls_config(tls(insecure).unwrap())
                .proxy(None)
                .timeout_global(Some(std::time::Duration::from_secs(5)))
                .build()
                .new_agent()
                .get(format!("https://{address}/"))
                .call();
            if insecure {
                assert_eq!(
                    result.unwrap().body_mut().read_to_string().unwrap(),
                    "trusted"
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    format!("{error:?}").contains("InvalidCertificate"),
                    "{error:?}"
                );
            }
            server.join().unwrap();
        }
    }
}
