use nu_protocol::ShellError;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

#[doc = include_str!("./tls.rustdoc.md")]
pub fn tls(allow_insecure: bool) -> Result<TlsConfig, ShellError> {
    Ok(TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .disable_verification(allow_insecure)
        .build())
}
