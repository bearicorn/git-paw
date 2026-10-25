//! Pure decision for the broker listener's browser-exposure guard.
//!
//! The broker is an unauthenticated HTTP server on the operator's loopback
//! interface. A web page the operator opens can reach it through DNS
//! rebinding — the attacker's hostname re-resolves to `127.0.0.1`, making the
//! page same-origin with `http://<attacker-host>:<port>` — and from there read
//! the whole coordination log or post an `agent.answer` that coding agents
//! treat as authoritative guidance.
//!
//! This module holds the decision that stops that, as a pure function so it
//! can be tested as a table. [`super::server::router`] applies it as a single
//! middleware layer over the whole listener, so it runs before routing and
//! covers every route plus the `404`/`405` fallbacks.

use std::net::IpAddr;

/// Why the listener guard refused a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginRejection {
    /// The `Host` header was missing, malformed, or named a non-loopback host
    /// while the broker is bound to a loopback address.
    ForbiddenHost,
    /// The `Origin` header named a non-loopback host, was the serialized value
    /// `null`, or could not be parsed.
    ForbiddenOrigin,
}

impl OriginRejection {
    /// The `error` string the broker returns in the `403` response body.
    #[must_use]
    pub fn error_message(self) -> &'static str {
        match self {
            Self::ForbiddenHost => "forbidden host",
            Self::ForbiddenOrigin => "forbidden origin",
        }
    }
}

/// How the listener sees one request header.
///
/// A header that is present but unusable is deliberately distinct from an
/// absent one: the guard's accept rule for `Origin` keys on absence, so an
/// `Origin` the listener cannot read must not be mistaken for a client that
/// sent none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeaderView<'a> {
    /// The header was not sent.
    #[default]
    Absent,
    /// The header was sent exactly once, with a value the listener can read.
    Value(&'a str),
    /// The header was sent more than once, or its bytes are not valid UTF-8.
    /// Either shape is a request-smuggling signal and is never accepted.
    Unusable,
}

/// Splits an HTTP authority into its host part and whatever follows it.
///
/// Handles `host`, `host:port`, `[v6]` and `[v6]:port`; the bracketed forms
/// yield the inner IPv6 literal. The trailing part includes the leading `:`
/// when a port is present, and is empty when it is not. Returns `None` for an
/// unterminated bracket.
fn split_authority(authority: &str) -> Option<(&str, &str)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let close = rest.find(']')?;
        Some((&rest[..close], &rest[close + 1..]))
    } else {
        match authority.find(':') {
            Some(index) => Some((&authority[..index], &authority[index..])),
            None => Some((authority, "")),
        }
    }
}

/// Returns `true` when what follows an authority's host part is a well-formed
/// port — either nothing at all, or `:` followed by at least one digit and
/// nothing else.
///
/// Without this, `localhost:evil.com`, `127.0.0.1:80:evil` and `[::1]evil.com`
/// would all be read as a loopback host with junk after it.
fn port_suffix_is_valid(suffix: &str) -> bool {
    match suffix.strip_prefix(':') {
        Some(port) => !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()),
        None => suffix.is_empty(),
    }
}

/// Returns `true` when a bare host name is one the guard accepts: `localhost`
/// (ASCII case-insensitive), or an IP literal whose
/// [`IpAddr::is_loopback`] is true (so all of `127.0.0.0/8` and `::1`).
fn is_loopback_name(name: &str) -> bool {
    if name.eq_ignore_ascii_case("localhost") {
        return true;
    }
    name.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Returns `true` when a whole authority (`host`, `host:port`, `[v6]:port`)
/// names a loopback host and carries nothing but a well-formed port after it.
///
/// The port's *value* is not checked — only its shape — so SSH tunnels and
/// local port-forwards on any port keep working.
fn is_loopback_authority(authority: &str) -> bool {
    let Some((host, suffix)) = split_authority(authority) else {
        return false;
    };
    !host.is_empty() && port_suffix_is_valid(suffix) && is_loopback_name(host)
}

/// Returns `true` when a serialized origin such as `http://localhost:3000`
/// names a loopback host.
///
/// Anything that is not `scheme://authority` is rejected, which includes the
/// serialized opaque origin `null`.
fn origin_is_loopback(origin: &str) -> bool {
    let Some((_scheme, rest)) = origin.split_once("://") else {
        return false;
    };
    let Some(authority) = rest.split(['/', '?', '#']).next() else {
        return false;
    };
    is_loopback_authority(authority)
}

/// Returns `true` when the configured `[broker] bind` address is a loopback
/// address, meaning the broker is only reachable from this machine and the
/// set of legitimate `Host` names is therefore known.
///
/// Accepts `localhost` and any IP literal whose [`IpAddr::is_loopback`] is
/// true; `0.0.0.0` and LAN addresses are not loopback.
#[must_use]
pub fn is_loopback_bind(bind: &str) -> bool {
    is_loopback_name(bind)
}

/// The warning printed on stderr when the broker is bound to a non-loopback
/// address.
///
/// Shared so the broker host process and the command that launches the
/// session print the same text. The launch command is the one an operator
/// actually reads: the host runs in a detached `broker` window, so anything
/// it writes to stderr goes unseen.
#[must_use]
pub fn non_loopback_bind_warning(bind: &str) -> String {
    format!(
        "warning: broker bound to {bind} — it is reachable from the network \
         without authentication, and the Host check that blocks DNS rebinding \
         is disabled. Any page a browser loads can then read the coordination \
         log. Use a loopback bind unless you control the network. See the \
         security-posture chapter of the git-paw user guide."
    )
}

/// Decides whether a request may be routed, from the addressing it presents
/// and whether the listener is bound to loopback.
///
/// On a loopback bind the `Host` header must be present exactly once, readable,
/// and name a loopback host followed by nothing or a well-formed port. The
/// port's value is deliberately not checked, so SSH tunnels and local
/// port-forwards keep working. `target_authority` is the authority of an
/// absolute-form request target (`GET http://host/path`), which RFC 9112 §3.2.2
/// makes authoritative over `Host`; when present it must pass the same rule, so
/// a foreign authority cannot be smuggled past a loopback `Host`.
///
/// On a non-loopback bind the whole `Host` half is skipped, because git-paw
/// cannot enumerate the operator's network names.
///
/// An `Origin` header, when present, must likewise name a loopback host — on
/// every bind, since that check is what keeps browsers out. A request with no
/// `Origin` header, which is what every non-browser client sends, is
/// unaffected by that half of the decision; a duplicated or unreadable
/// `Origin` is refused rather than treated as absent.
///
/// # Errors
///
/// Returns [`OriginRejection::ForbiddenHost`] or
/// [`OriginRejection::ForbiddenOrigin`] for a request the listener must refuse
/// with `403`.
pub fn decide_request_origin(
    host: HeaderView<'_>,
    origin: HeaderView<'_>,
    target_authority: Option<&str>,
    bind_is_loopback: bool,
) -> Result<(), OriginRejection> {
    if bind_is_loopback {
        match host {
            HeaderView::Value(value) if is_loopback_authority(value) => {}
            _ => return Err(OriginRejection::ForbiddenHost),
        }
        if let Some(authority) = target_authority
            && !is_loopback_authority(authority)
        {
            return Err(OriginRejection::ForbiddenHost);
        }
    }
    match origin {
        HeaderView::Absent => {}
        HeaderView::Value(value) if origin_is_loopback(value) => {}
        _ => return Err(OriginRejection::ForbiddenOrigin),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shorthand for the common "only a Host header" case.
    fn host_only(host: &str, bind_is_loopback: bool) -> Result<(), OriginRejection> {
        decide_request_origin(
            HeaderView::Value(host),
            HeaderView::Absent,
            None,
            bind_is_loopback,
        )
    }

    #[test]
    fn loopback_bind_accepts_loopback_hosts() {
        for host in [
            "127.0.0.1:9119",
            "127.3.4.5",
            "localhost:19119",
            "LOCALHOST",
            "[::1]:9119",
            "[::1]",
        ] {
            assert_eq!(
                host_only(host, true),
                Ok(()),
                "host {host} should be accepted on a loopback bind"
            );
        }
    }

    #[test]
    fn loopback_bind_refuses_foreign_and_missing_hosts() {
        for host in [
            "attacker.example:9119",
            "192.168.1.5",
            "localhost.attacker.example",
            // A trailing dot makes it a different name; it is not `localhost`.
            "localhost.",
            "",
        ] {
            assert_eq!(
                host_only(host, true),
                Err(OriginRejection::ForbiddenHost),
                "host {host:?} should be refused on a loopback bind"
            );
        }
        assert_eq!(
            decide_request_origin(HeaderView::Absent, HeaderView::Absent, None, true),
            Err(OriginRejection::ForbiddenHost),
            "a missing Host must be refused on a loopback bind"
        );
    }

    #[test]
    fn loopback_bind_refuses_junk_after_the_host_part() {
        // Each of these starts with an accepted host name; only the port
        // suffix check tells them apart from the real thing.
        for host in [
            "localhost:evil.com",
            "127.0.0.1:80:evil",
            "[::1]evil.com",
            "127.0.0.1:",
            "localhost:9119x",
            "[::1]:evil",
        ] {
            assert_eq!(
                host_only(host, true),
                Err(OriginRejection::ForbiddenHost),
                "host {host:?} must not be read as a bare loopback host"
            );
        }
    }

    #[test]
    fn a_duplicated_or_unreadable_host_is_refused() {
        assert_eq!(
            decide_request_origin(HeaderView::Unusable, HeaderView::Absent, None, true),
            Err(OriginRejection::ForbiddenHost),
            "a duplicated or non-UTF-8 Host must be refused, never read as the first value"
        );
    }

    #[test]
    fn a_duplicated_or_unreadable_origin_is_refused_on_every_bind() {
        for bind_is_loopback in [true, false] {
            assert_eq!(
                decide_request_origin(
                    HeaderView::Value("127.0.0.1:9119"),
                    HeaderView::Unusable,
                    None,
                    bind_is_loopback
                ),
                Err(OriginRejection::ForbiddenOrigin),
                "a duplicated or non-UTF-8 Origin must be refused, never treated as absent"
            );
        }
    }

    #[test]
    fn origin_is_checked_on_every_bind() {
        for bind_is_loopback in [true, false] {
            assert_eq!(
                decide_request_origin(
                    HeaderView::Value("127.0.0.1:9119"),
                    HeaderView::Absent,
                    None,
                    bind_is_loopback
                ),
                Ok(()),
                "an absent Origin is always accepted"
            );
            for origin in [
                "http://localhost:3000",
                "http://127.0.0.1",
                "http://[::1]:8080",
            ] {
                assert_eq!(
                    decide_request_origin(
                        HeaderView::Value("127.0.0.1:9119"),
                        HeaderView::Value(origin),
                        None,
                        bind_is_loopback
                    ),
                    Ok(()),
                    "loopback origin {origin} should be accepted"
                );
            }
            for origin in [
                "http://attacker.example",
                "null",
                "http://192.168.1.5:80",
                "http://localhost:evil.com",
                "http://localhost.attacker.example",
                "",
            ] {
                assert_eq!(
                    decide_request_origin(
                        HeaderView::Value("127.0.0.1:9119"),
                        HeaderView::Value(origin),
                        None,
                        bind_is_loopback
                    ),
                    Err(OriginRejection::ForbiddenOrigin),
                    "origin {origin:?} should be refused"
                );
            }
        }
    }

    #[test]
    fn an_absolute_form_target_must_also_be_loopback() {
        // `GET http://evil.com/status` with a loopback `Host`: the request
        // target's authority is authoritative, so it must pass too.
        for authority in ["evil.com", "evil.com:9119", "localhost:evil.com"] {
            assert_eq!(
                decide_request_origin(
                    HeaderView::Value("127.0.0.1:9119"),
                    HeaderView::Absent,
                    Some(authority),
                    true
                ),
                Err(OriginRejection::ForbiddenHost),
                "absolute-form authority {authority:?} should be refused"
            );
        }
        for authority in ["127.0.0.1:9119", "localhost", "[::1]:9119"] {
            assert_eq!(
                decide_request_origin(
                    HeaderView::Value("127.0.0.1:9119"),
                    HeaderView::Absent,
                    Some(authority),
                    true
                ),
                Ok(()),
                "absolute-form authority {authority} should be accepted"
            );
        }
    }

    #[test]
    fn non_loopback_bind_skips_the_host_check() {
        for host in [
            HeaderView::Value("192.168.1.5:9119"),
            HeaderView::Value("attacker.example:9119"),
            HeaderView::Value("0.0.0.0:9119"),
            HeaderView::Absent,
            HeaderView::Unusable,
        ] {
            assert_eq!(
                decide_request_origin(host, HeaderView::Absent, None, false),
                Ok(()),
                "host {host:?} should be accepted on a non-loopback bind"
            );
        }
        // The absolute-form check rides on the same bind rule.
        assert_eq!(
            decide_request_origin(
                HeaderView::Value("192.168.1.5:9119"),
                HeaderView::Absent,
                Some("evil.com"),
                false
            ),
            Ok(())
        );
    }

    #[test]
    fn is_loopback_bind_classifies_configured_addresses() {
        for bind in ["127.0.0.1", "127.0.0.2", "::1", "localhost", "LocalHost"] {
            assert!(is_loopback_bind(bind), "{bind} should be loopback");
        }
        for bind in ["0.0.0.0", "192.168.1.5", "::", "example.com", "localhost."] {
            assert!(!is_loopback_bind(bind), "{bind} should not be loopback");
        }
    }

    #[test]
    fn rejections_carry_the_documented_error_strings() {
        assert_eq!(
            OriginRejection::ForbiddenHost.error_message(),
            "forbidden host"
        );
        assert_eq!(
            OriginRejection::ForbiddenOrigin.error_message(),
            "forbidden origin"
        );
    }
}
