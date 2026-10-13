use std::net::IpAddr;

use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue};

/// Set by Cloudflare on every proxied request, Cloudflare Tunnel included.
pub const CLIENT_ADDRESS_HEADER: HeaderName = HeaderName::from_static("cf-connecting-ip");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClientAddressHeaderErrorKind {
    Missing,
    Repeated,
    Unparseable,
}

impl ClientAddressHeaderErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            ClientAddressHeaderErrorKind::Missing => "missing",
            ClientAddressHeaderErrorKind::Repeated => "repeated",
            ClientAddressHeaderErrorKind::Unparseable => "unparseable",
        }
    }
}

/// The `CF-Connecting-IP` address when the peer is a trusted proxy and sent it exactly once; otherwise the peer.
pub fn resolve_client_ip(peer_ip: IpAddr, trusted_proxy_addresses: &[IpAddr], headers: &HeaderMap) -> IpAddr {
    let is_peer_trusted: bool = is_trusted_proxy(trusted_proxy_addresses, peer_ip);

    if !is_peer_trusted {
        return peer_ip;
    }

    let header_client_ip: Result<IpAddr, ClientAddressHeaderErrorKind> = read_client_address_header(headers);

    match header_client_ip {
        Ok(client_ip) => client_ip,
        Err(error_kind) => {
            log::warn!(
                "a trusted proxy sent no valid client address header; [peer={peer_ip} reason={}]",
                error_kind.as_str(),
            );

            peer_ip
        }
    }
}

/// A dual-stack listener reports an IPv4 peer as its IPv4-mapped IPv6 address.
fn is_trusted_proxy(trusted_proxy_addresses: &[IpAddr], peer_ip: IpAddr) -> bool {
    let canonical_peer_ip: IpAddr = peer_ip.to_canonical();

    trusted_proxy_addresses
        .iter()
        .any(|trusted_proxy_address: &IpAddr| trusted_proxy_address.to_canonical() == canonical_peer_ip)
}

fn read_client_address_header(headers: &HeaderMap) -> Result<IpAddr, ClientAddressHeaderErrorKind> {
    let mut header_values: std::slice::Iter<'_, HeaderValue> = headers.get_all(CLIENT_ADDRESS_HEADER);

    let Some(header_value) = header_values.next() else {
        return Err(ClientAddressHeaderErrorKind::Missing);
    };

    if header_values.next().is_some() {
        return Err(ClientAddressHeaderErrorKind::Repeated);
    }

    header_value
        .to_str()
        .ok()
        .and_then(|header_text: &str| header_text.parse::<IpAddr>().ok())
        .ok_or(ClientAddressHeaderErrorKind::Unparseable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    const PROXY_IP: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
    const FORWARDED_IPV4: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7));
    const FORWARDED_IPV6: IpAddr = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 7));

    fn create_headers(client_address_header_values: &[&'static str]) -> HeaderMap {
        let mut headers: HeaderMap = HeaderMap::new();

        for header_value in client_address_header_values {
            headers.append(CLIENT_ADDRESS_HEADER, HeaderValue::from_static(header_value));
        }

        headers
    }

    #[test]
    fn resolve_client_ip_ignores_the_header_from_an_untrusted_peer() {
        let peer_ip: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 9));
        let headers: HeaderMap = create_headers(&["198.51.100.7"]);

        assert_eq!(resolve_client_ip(peer_ip, &[PROXY_IP], &headers), peer_ip);
    }

    #[test]
    fn resolve_client_ip_ignores_the_header_when_no_proxy_is_trusted() {
        let headers: HeaderMap = create_headers(&["198.51.100.7"]);

        assert_eq!(resolve_client_ip(PROXY_IP, &[], &headers), PROXY_IP);
    }

    #[test]
    fn resolve_client_ip_reads_an_ipv4_or_ipv6_header_from_a_trusted_peer() {
        assert_eq!(
            resolve_client_ip(PROXY_IP, &[PROXY_IP], &create_headers(&["198.51.100.7"])),
            FORWARDED_IPV4,
        );
        assert_eq!(
            resolve_client_ip(PROXY_IP, &[PROXY_IP], &create_headers(&["2001:db8::7"])),
            FORWARDED_IPV6,
        );
    }

    #[test]
    fn resolve_client_ip_trusts_an_ipv4_mapped_peer_as_its_ipv4_address() {
        let mapped_peer_ip: IpAddr = IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped());
        let headers: HeaderMap = create_headers(&["198.51.100.7"]);

        assert_eq!(resolve_client_ip(mapped_peer_ip, &[PROXY_IP], &headers), FORWARDED_IPV4);
    }

    #[test]
    fn resolve_client_ip_falls_back_to_the_peer_without_a_valid_single_header() {
        let invalid_header_value_lists: [&[&'static str]; 5] = [
            &[],
            &[""],
            &["not an address"],
            &["198.51.100.7, 198.51.100.8"],
            &["198.51.100.7", "198.51.100.8"],
        ];

        for header_values in invalid_header_value_lists {
            assert_eq!(
                resolve_client_ip(PROXY_IP, &[PROXY_IP], &create_headers(header_values)),
                PROXY_IP,
            );
        }
    }
}
