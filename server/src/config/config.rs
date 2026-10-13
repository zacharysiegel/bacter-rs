use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use shared::error::AppError;
use shared::game::tick;

pub const EMPTY_GAME_GRACE_PERIOD: Duration = Duration::from_secs(10);
pub const BIND_ADDRESS_VARIABLE: &str = "BACTER_BIND_ADDRESS";
pub const PORT_VARIABLE: &str = "BACTER_PORT";
pub const SITE_ROOT_VARIABLE: &str = "BACTER_SITE_ROOT";
pub const ALLOWED_ORIGINS_VARIABLE: &str = "BACTER_ALLOWED_ORIGINS";
pub const REPLAY_CAPTURE_DIRECTORY_VARIABLE: &str = "BACTER_REPLAY_CAPTURE_DIRECTORY";
pub const TLS_CERTIFICATE_PATH_VARIABLE: &str = "BACTER_TLS_CERTIFICATE_PATH";
pub const TLS_PRIVATE_KEY_PATH_VARIABLE: &str = "BACTER_TLS_PRIVATE_KEY_PATH";
pub const TRUSTED_PROXY_ADDRESSES_VARIABLE: &str = "BACTER_TRUSTED_PROXY_ADDRESSES";
const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3100;
/// The web crate's `site-addr`.
const DEFAULT_ALLOWED_ORIGINS: &str = "http://127.0.0.1:3011,http://localhost:3011";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    pub bind_address: IpAddr,
    /// Zero binds any free port.
    pub port: u16,
    /// None serves no static files.
    pub site_root: Option<PathBuf>,
    /// Page origins allowed to open `/ws`.
    pub allowed_origins: Vec<String>,
    pub replay_capture_directory: Option<PathBuf>,
    /// None binds plain HTTP.
    pub tls_files: Option<TlsFiles>,
    /// Peers whose `CF-Connecting-IP` header is read as the client address; empty reads it from no peer.
    pub trusted_proxy_addresses: Vec<IpAddr>,
    pub empty_game_grace_period: Duration,
    pub tick_period: Duration,
}

impl ServerConfig {
    pub fn from_environment() -> Result<ServerConfig, AppError> {
        ServerConfig::from_variables(|name: &str| std::env::var(name).ok())
    }

    /// An empty variable counts as unset.
    pub fn from_variables(get_variable: impl Fn(&str) -> Option<String>) -> Result<ServerConfig, AppError> {
        let get_set_variable = |name: &str| get_variable(name).filter(|value: &String| !value.trim().is_empty());

        let bind_address_text: String =
            get_set_variable(BIND_ADDRESS_VARIABLE).unwrap_or_else(|| String::from(DEFAULT_BIND_ADDRESS));
        let bind_address: IpAddr = parse_variable(BIND_ADDRESS_VARIABLE, &bind_address_text)?;
        let port: u16 = match get_set_variable(PORT_VARIABLE) {
            Some(port_text) => parse_variable(PORT_VARIABLE, &port_text)?,
            None => DEFAULT_PORT,
        };
        let allowed_origins_text: String =
            get_set_variable(ALLOWED_ORIGINS_VARIABLE).unwrap_or_else(|| String::from(DEFAULT_ALLOWED_ORIGINS));
        let tls_files: Option<TlsFiles> = get_tls_files(
            get_set_variable(TLS_CERTIFICATE_PATH_VARIABLE),
            get_set_variable(TLS_PRIVATE_KEY_PATH_VARIABLE),
        )?;
        let trusted_proxy_addresses: Vec<IpAddr> = match get_set_variable(TRUSTED_PROXY_ADDRESSES_VARIABLE) {
            Some(trusted_proxy_addresses_text) => split_trusted_proxy_addresses(&trusted_proxy_addresses_text)?,
            None => Vec::new(),
        };

        Ok(ServerConfig {
            bind_address,
            port,
            site_root: get_set_variable(SITE_ROOT_VARIABLE).map(PathBuf::from),
            allowed_origins: split_origins(&allowed_origins_text),
            replay_capture_directory: get_set_variable(REPLAY_CAPTURE_DIRECTORY_VARIABLE).map(PathBuf::from),
            tls_files,
            trusted_proxy_addresses,
            empty_game_grace_period: EMPTY_GAME_GRACE_PERIOD,
            tick_period: Duration::from_millis(u64::from(tick::TICK_PERIOD_MILLISECONDS)),
        })
    }
}

/// PEM files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlsFiles {
    pub certificate_chain_path: PathBuf,
    pub private_key_path: PathBuf,
}

/// Exact match.
pub fn is_origin_allowed(allowed_origins: &[String], origin: &str) -> bool {
    allowed_origins.iter().any(|allowed_origin: &String| allowed_origin == origin)
}

fn get_tls_files(
    certificate_path: Option<String>,
    private_key_path: Option<String>,
) -> Result<Option<TlsFiles>, AppError> {
    match (certificate_path, private_key_path) {
        (Some(certificate_path), Some(private_key_path)) => Ok(Some(TlsFiles {
            certificate_chain_path: PathBuf::from(certificate_path),
            private_key_path: PathBuf::from(private_key_path),
        })),
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(AppError::new(&format!(
            "set both {TLS_CERTIFICATE_PATH_VARIABLE} and {TLS_PRIVATE_KEY_PATH_VARIABLE}, or neither"
        ))),
    }
}

fn parse_variable<T: FromStr>(name: &str, value: &str) -> Result<T, AppError> {
    value.trim().parse::<T>().map_err(|_| AppError::new(&format!("{name} is not valid: {value}")))
}

/// Trailing slashes are dropped; a browser's `Origin` has none.
fn split_origins(allowed_origins_text: &str) -> Vec<String> {
    allowed_origins_text
        .split(',')
        .map(|origin: &str| origin.trim().trim_end_matches('/'))
        .filter(|origin: &&str| !origin.is_empty())
        .map(String::from)
        .collect()
}

fn split_trusted_proxy_addresses(trusted_proxy_addresses_text: &str) -> Result<Vec<IpAddr>, AppError> {
    trusted_proxy_addresses_text
        .split(',')
        .map(|trusted_proxy_address_text: &str| {
            parse_variable::<IpAddr>(TRUSTED_PROXY_ADDRESSES_VARIABLE, trusted_proxy_address_text)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn from_pairs(pairs: &[(&str, &str)]) -> Result<ServerConfig, AppError> {
        let variables: BTreeMap<String, String> =
            pairs.iter().map(|(name, value)| (String::from(*name), String::from(*value))).collect();

        ServerConfig::from_variables(|name: &str| variables.get(name).cloned())
    }

    #[test]
    fn from_variables_uses_the_defaults_when_nothing_is_set() {
        let config: ServerConfig = from_pairs(&[]).unwrap();

        assert_eq!(
            config,
            ServerConfig {
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 3100,
                site_root: None,
                allowed_origins: vec![
                    String::from("http://127.0.0.1:3011"),
                    String::from("http://localhost:3011"),
                ],
                replay_capture_directory: None,
                tls_files: None,
                trusted_proxy_addresses: Vec::new(),
                empty_game_grace_period: Duration::from_secs(10),
                tick_period: Duration::from_millis(70),
            },
        );
    }

    #[test]
    fn from_variables_reads_every_variable() {
        let config: ServerConfig = from_pairs(&[
            (BIND_ADDRESS_VARIABLE, "0.0.0.0"),
            (PORT_VARIABLE, "443"),
            (SITE_ROOT_VARIABLE, "target/site"),
            (
                ALLOWED_ORIGINS_VARIABLE,
                " https://bacter.example/ , https://www.bacter.example",
            ),
            (REPLAY_CAPTURE_DIRECTORY_VARIABLE, "/tmp/bacter/replays"),
            (TLS_CERTIFICATE_PATH_VARIABLE, "/etc/bacter/chain.pem"),
            (TLS_PRIVATE_KEY_PATH_VARIABLE, "/etc/bacter/key.pem"),
            (TRUSTED_PROXY_ADDRESSES_VARIABLE, " 127.0.0.1 , ::1"),
        ])
        .unwrap();

        assert_eq!(config.bind_address, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert_eq!(config.port, 443);
        assert_eq!(config.site_root, Some(PathBuf::from("target/site")));
        assert_eq!(
            config.allowed_origins,
            vec![
                String::from("https://bacter.example"),
                String::from("https://www.bacter.example"),
            ],
        );
        assert_eq!(
            config.replay_capture_directory,
            Some(PathBuf::from("/tmp/bacter/replays"))
        );
        assert_eq!(
            config.tls_files,
            Some(TlsFiles {
                certificate_chain_path: PathBuf::from("/etc/bacter/chain.pem"),
                private_key_path: PathBuf::from("/etc/bacter/key.pem"),
            }),
        );
        assert_eq!(
            config.trusted_proxy_addresses,
            vec![IpAddr::V4(Ipv4Addr::LOCALHOST), IpAddr::V6(Ipv6Addr::LOCALHOST)],
        );
    }

    #[test]
    fn from_variables_treats_an_empty_variable_as_unset() {
        let config: ServerConfig = from_pairs(&[(SITE_ROOT_VARIABLE, ""), (PORT_VARIABLE, " ")]).unwrap();

        assert_eq!(config.site_root, None);
        assert_eq!(config.port, 3100);
    }

    #[test]
    fn from_variables_rejects_only_one_tls_variable() {
        assert!(from_pairs(&[(TLS_CERTIFICATE_PATH_VARIABLE, "/etc/bacter/chain.pem")]).is_err());
        assert!(from_pairs(&[(TLS_PRIVATE_KEY_PATH_VARIABLE, "/etc/bacter/key.pem")]).is_err());
    }

    #[test]
    fn from_variables_rejects_an_invalid_port_or_address() {
        assert!(from_pairs(&[(PORT_VARIABLE, "70000")]).is_err());
        assert!(from_pairs(&[(BIND_ADDRESS_VARIABLE, "localhost")]).is_err());
    }

    #[test]
    fn from_variables_rejects_an_invalid_trusted_proxy_address() {
        assert!(from_pairs(&[(TRUSTED_PROXY_ADDRESSES_VARIABLE, "127.0.0.1,localhost")]).is_err());
        assert!(from_pairs(&[(TRUSTED_PROXY_ADDRESSES_VARIABLE, "127.0.0.1,,::1")]).is_err());
        assert!(from_pairs(&[(TRUSTED_PROXY_ADDRESSES_VARIABLE, "127.0.0.0/8")]).is_err());
    }

    #[test]
    fn is_origin_allowed_matches_exactly() {
        let config: ServerConfig = from_pairs(&[]).unwrap();

        assert!(is_origin_allowed(&config.allowed_origins, "http://localhost:3011"));
        assert!(!is_origin_allowed(&config.allowed_origins, "http://localhost:3012"));
        assert!(!is_origin_allowed(&config.allowed_origins, "http://evil.example"));
    }
}
