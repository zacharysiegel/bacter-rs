use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use shared::error::AppError;

use crate::config::TlsFiles;

pub fn create_rustls_config(tls_files: &TlsFiles) -> Result<rustls::ServerConfig, AppError> {
    let certificate_chain: Vec<CertificateDer<'static>> = read_certificate_chain(tls_files)?;
    let private_key: PrivateKeyDer<'static> =
        PrivateKeyDer::from_pem_file(&tls_files.private_key_path).map_err(|error| {
            AppError::from_error(
                &format!("cannot read the private key {}", tls_files.private_key_path.display()),
                Box::new(error),
            )
        })?;

    rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificate_chain, private_key)
        .map_err(|error| AppError::from_error("the certificate chain does not match the private key", Box::new(error)))
}

fn read_certificate_chain(tls_files: &TlsFiles) -> Result<Vec<CertificateDer<'static>>, AppError> {
    let certificate_error = |error: rustls::pki_types::pem::Error| {
        AppError::from_error(
            &format!(
                "cannot read the certificate chain {}",
                tls_files.certificate_chain_path.display()
            ),
            Box::new(error),
        )
    };

    let certificate_chain: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(&tls_files.certificate_chain_path)
            .map_err(certificate_error)?
            .collect::<Result<Vec<CertificateDer<'static>>, rustls::pki_types::pem::Error>>()
            .map_err(certificate_error)?;

    if certificate_chain.is_empty() {
        return Err(AppError::new(&format!(
            "the certificate chain {} holds no certificate",
            tls_files.certificate_chain_path.display()
        )));
    }

    Ok(certificate_chain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_fixture_path(file_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(file_name)
    }

    #[test]
    fn create_rustls_config_reads_the_certificate_chain_and_private_key() {
        let tls_files: TlsFiles = TlsFiles {
            certificate_chain_path: get_fixture_path("localhost.certificate.pem"),
            private_key_path: get_fixture_path("localhost.key.pem"),
        };

        assert!(create_rustls_config(&tls_files).is_ok());
    }

    #[test]
    fn create_rustls_config_rejects_a_missing_file() {
        let tls_files: TlsFiles = TlsFiles {
            certificate_chain_path: get_fixture_path("missing.pem"),
            private_key_path: get_fixture_path("localhost.key.pem"),
        };

        assert!(create_rustls_config(&tls_files).is_err());
    }

    #[test]
    fn create_rustls_config_rejects_a_key_as_the_certificate_chain() {
        let tls_files: TlsFiles = TlsFiles {
            certificate_chain_path: get_fixture_path("localhost.key.pem"),
            private_key_path: get_fixture_path("localhost.key.pem"),
        };

        assert!(create_rustls_config(&tls_files).is_err());
    }
}
