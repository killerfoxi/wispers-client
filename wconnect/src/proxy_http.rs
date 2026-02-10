//! HTTP proxy for accessing web servers on remote nodes.
//!
//! This module implements a forward HTTP proxy that allows browsers/clients
//! to access web servers running on nodes in the connectivity group using
//! hostnames like `http://3.wispers.link/`.

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wispers_connect::{Node, NodeState, QuicConnection};

use crate::proxy_common::{
    parse_wispers_host, ConnectionPool, ProxyError, CLEANUP_INTERVAL, REQUEST_TIMEOUT,
};

/// Run the HTTP proxy server.
pub async fn run(hub_override: Option<&str>, profile: &str, bind_addr: &str) -> Result<()> {
    let storage = super::get_storage(hub_override, profile)?;
    let node = storage
        .restore_or_init_node()
        .await
        .context("failed to load node state")?;

    if node.state() != NodeState::Activated {
        anyhow::bail!(
            "Node must be activated to use HTTP proxy. Current state: {:?}",
            node.state()
        );
    }

    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind to {}", bind_addr))?;

    println!("HTTP proxy listening on {}", bind_addr);
    println!("Configure your browser/client to use this as HTTP proxy");
    println!("Example: curl --proxy http://{} http://3.wispers.link/", bind_addr);

    let node = Arc::new(node);
    let pool = ConnectionPool::new();

    // Start background cleanup task
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(CLEANUP_INTERVAL).await;
            cleanup_pool.cleanup_idle().await;
        }
    });

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("Accepted connection from {}", addr);
                let node = Arc::clone(&node);
                let pool = pool.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, node, pool).await {
                        eprintln!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }
}

/// Parsed proxy request target.
#[derive(Debug)]
struct ProxyTarget {
    /// Target node number
    node_number: i32,
    /// Target port (default 80)
    port: u16,
    /// Path including query string (e.g., "/path?query=1")
    path: String,
}

/// Parsed HTTP request ready for forwarding.
#[derive(Debug)]
struct ParsedRequest {
    /// The proxy target extracted from the URL
    target: ProxyTarget,
    /// The original request method
    method: String,
    /// HTTP version (0 for HTTP/1.0, 1 for HTTP/1.1)
    version: u8,
    /// Raw headers to forward (excluding hop-by-hop headers)
    headers: Vec<(String, String)>,
    /// Whether to keep the connection alive
    keep_alive: bool,
}

/// Handle a single client connection (may process multiple requests via keep-alive).
async fn handle_connection(
    mut stream: TcpStream,
    node: Arc<Node>,
    pool: ConnectionPool,
) -> Result<()> {
    let peer = stream.peer_addr()?;
    let mut request_count = 0;

    loop {
        // Read HTTP request bytes
        let buf = match read_request_bytes(&mut stream).await {
            Ok(ReadResult::Data(buf)) => buf,
            Ok(ReadResult::Closed) => {
                // Client closed connection gracefully
                break;
            }
            Err(e) => {
                if request_count == 0 {
                    // First request - send error response
                    send_proxy_error(&mut stream, &e).await?;
                }
                break;
            }
        };

        // Parse the request
        let request = match parse_request(&buf) {
            Ok(req) => req,
            Err(e) => {
                send_proxy_error(&mut stream, &e).await?;
                break;
            }
        };

        request_count += 1;
        let keep_alive = request.keep_alive;

        println!(
            "  {} -> node {}:{}{} (keep-alive: {})",
            request.method, request.target.node_number, request.target.port,
            request.target.path, keep_alive
        );

        // Get or create QUIC connection to target node (with timeout)
        let quic_conn = match tokio::time::timeout(
            REQUEST_TIMEOUT,
            pool.get_or_connect(&node, request.target.node_number),
        )
        .await
        {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                let err = ProxyError::BadGateway(format!("failed to connect to node: {}", e));
                send_proxy_error(&mut stream, &err).await?;
                break;
            }
            Err(_) => {
                let err = ProxyError::GatewayTimeout("connection to node timed out".to_string());
                send_proxy_error(&mut stream, &err).await?;
                break;
            }
        };

        // Forward the request (with timeout)
        match tokio::time::timeout(
            REQUEST_TIMEOUT,
            forward_request(&mut stream, &quic_conn, &request),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                eprintln!("  Forward error: {}", e);
                // Don't send error response - we may have already started sending the response
                break;
            }
            Err(_) => {
                // Timeout - try to send error if we haven't started the response
                let err = ProxyError::GatewayTimeout("request timed out".to_string());
                let _ = send_proxy_error(&mut stream, &err).await;
                break;
            }
        }

        // If not keep-alive, close the connection
        if !keep_alive {
            break;
        }

        // Otherwise, loop back to handle the next request
    }

    println!(
        "Connection from {} closed ({} request{})",
        peer,
        request_count,
        if request_count == 1 { "" } else { "s" }
    );
    Ok(())
}

/// Result of reading HTTP request bytes from the stream.
enum ReadResult {
    /// Got complete request bytes
    Data(Vec<u8>),
    /// Client closed connection gracefully (no data)
    Closed,
}

/// Read HTTP request bytes from the stream.
async fn read_request_bytes(stream: &mut TcpStream) -> Result<ReadResult, ProxyError> {
    let mut buf = vec![0u8; 8192];
    let mut total_read = 0;

    loop {
        if total_read >= buf.len() {
            return Err(ProxyError::BadRequest("request too large".to_string()));
        }

        let n = stream.read(&mut buf[total_read..]).await.map_err(|e| {
            ProxyError::BadRequest(format!("failed to read request: {}", e))
        })?;

        if n == 0 {
            if total_read == 0 {
                // Client closed connection before sending anything
                return Ok(ReadResult::Closed);
            }
            return Err(ProxyError::BadRequest(
                "connection closed before complete request".to_string(),
            ));
        }
        total_read += n;

        // Check if we have a complete request (ends with \r\n\r\n)
        if total_read >= 4 {
            let data = &buf[..total_read];
            if data.windows(4).any(|w| w == b"\r\n\r\n") {
                buf.truncate(total_read);
                return Ok(ReadResult::Data(buf));
            }
        }
    }
}

/// Forward an HTTP request through a QUIC stream to the target node.
async fn forward_request(
    client_stream: &mut TcpStream,
    quic_conn: &QuicConnection,
    request: &ParsedRequest,
) -> Result<()> {
    // Open a new stream for this request
    let quic_stream = quic_conn
        .open_stream()
        .await
        .context("failed to open QUIC stream")?;

    // Send FORWARD command
    let forward_cmd = format!("FORWARD {}\n", request.target.port);
    quic_stream
        .write_all(forward_cmd.as_bytes())
        .await
        .context("failed to send FORWARD command")?;

    // Read response (OK or ERROR)
    let mut response_buf = [0u8; 256];
    let n = quic_stream
        .read(&mut response_buf)
        .await
        .context("failed to read FORWARD response")?;

    let response = String::from_utf8_lossy(&response_buf[..n]);
    let response = response.trim();

    if response.starts_with("ERROR ") {
        let error_msg = &response[6..];
        send_error(client_stream, 502, &format!("Remote error: {}", error_msg)).await?;
        return Ok(());
    }

    if response != "OK" {
        send_error(client_stream, 502, &format!("Unexpected response: {}", response)).await?;
        return Ok(());
    }

    // Build and send the HTTP request to the remote server
    let http_request = build_http_request(request);
    quic_stream
        .write_all(http_request.as_bytes())
        .await
        .context("failed to send HTTP request")?;

    // Relay the response back to the client
    // We read from the QUIC stream and write to the client TCP stream
    let mut buf = [0u8; 8192];
    loop {
        let n = quic_stream
            .read(&mut buf)
            .await
            .context("failed to read from remote")?;

        if n == 0 {
            break;
        }

        client_stream
            .write_all(&buf[..n])
            .await
            .context("failed to write to client")?;
    }

    Ok(())
}

/// Build an HTTP request string from the parsed request.
fn build_http_request(request: &ParsedRequest) -> String {
    let mut http = String::new();

    // Request line: METHOD /path HTTP/1.1
    let version = if request.version == 0 { "1.0" } else { "1.1" };
    http.push_str(&format!(
        "{} {} HTTP/{}\r\n",
        request.method, request.target.path, version
    ));

    // Headers
    for (name, value) in &request.headers {
        http.push_str(&format!("{}: {}\r\n", name, value));
    }

    // End of headers
    http.push_str("\r\n");

    http
}

/// Parse an HTTP request from a buffer.
fn parse_request(buf: &[u8]) -> Result<ParsedRequest, ProxyError> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);

    let status = req.parse(buf).map_err(|e| {
        ProxyError::BadRequest(format!("failed to parse HTTP request: {}", e))
    })?;
    if status.is_partial() {
        return Err(ProxyError::BadRequest("incomplete HTTP request".to_string()));
    }

    let method = req
        .method
        .ok_or_else(|| ProxyError::BadRequest("missing method".to_string()))?
        .to_string();
    let path = req
        .path
        .ok_or_else(|| ProxyError::BadRequest("missing path".to_string()))?;
    let version = req
        .version
        .ok_or_else(|| ProxyError::BadRequest("missing version".to_string()))?;

    // Parse the target from the absolute URL
    let target = parse_proxy_target(path)?;

    // Collect headers, filtering out hop-by-hop headers
    let mut parsed_headers = Vec::new();
    let mut keep_alive = version == 1; // HTTP/1.1 defaults to keep-alive
    let mut host_header = None;

    for header in req.headers.iter() {
        let name = header.name.to_lowercase();
        let value = String::from_utf8_lossy(header.value).to_string();

        // Check Connection header for keep-alive
        if name == "connection" {
            keep_alive = value.to_lowercase().contains("keep-alive");
            // Don't forward Connection header as-is
            continue;
        }

        // Skip other hop-by-hop headers
        if is_hop_by_hop_header(&name) {
            continue;
        }

        if name == "host" {
            host_header = Some(value.clone());
        }

        parsed_headers.push((header.name.to_string(), value));
    }

    // If Host header is missing, add it from the target
    if host_header.is_none() {
        let host = if target.port == 80 {
            format!("{}.wispers.link", target.node_number)
        } else {
            format!("{}.wispers.link:{}", target.node_number, target.port)
        };
        parsed_headers.push(("Host".to_string(), host));
    }

    Ok(ParsedRequest {
        target,
        method,
        version,
        headers: parsed_headers,
        keep_alive,
    })
}

/// Parse the proxy target from an absolute URL.
///
/// Expected format: `http://<node_number>.wispers.link[:port]/path`
fn parse_proxy_target(url: &str) -> Result<ProxyTarget, ProxyError> {
    // Must start with http://
    let rest = match url.strip_prefix("http://") {
        Some(r) => r,
        None => {
            return Err(ProxyError::BadRequest(
                "proxy requests must use absolute URLs (http://...)".to_string(),
            ));
        }
    };

    // Split host and path
    let (host_port, path) = match rest.find('/') {
        Some(pos) => (&rest[..pos], &rest[pos..]),
        None => (rest, "/"),
    };

    // Parse host and optional port
    let (host, port) = match host_port.rfind(':') {
        Some(pos) => {
            let port_str = &host_port[pos + 1..];
            let port: u16 = port_str.parse().map_err(|_| {
                ProxyError::BadRequest(format!("invalid port: {}", port_str))
            })?;
            (&host_port[..pos], port)
        }
        None => (host_port, 80),
    };

    // Validate hostname is wispers.link and parse node number
    let node_number = match parse_wispers_host(host) {
        Ok(wispers_host) => wispers_host.node_number,
        Err(None) => {
            // Not a wispers.link hostname - forbidden (no egress support yet)
            return Err(ProxyError::Forbidden(format!(
                "only *.wispers.link hosts are allowed, got: {}",
                host
            )));
        }
        Err(Some(e)) => return Err(e),
    };

    Ok(ProxyTarget {
        node_number,
        port,
        path: path.to_string(),
    })
}

/// Check if a header is a hop-by-hop header that shouldn't be forwarded.
fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}

/// Send an HTTP error response.
/// Send an HTTP error response for a ProxyError.
async fn send_proxy_error(stream: &mut TcpStream, error: &ProxyError) -> Result<()> {
    send_error(stream, error.status_code(), &error.to_string()).await
}

/// Send an HTTP error response.
async fn send_error(stream: &mut TcpStream, status: u16, message: &str) -> Result<()> {
    let status_text = match status {
        400 => "Bad Request",
        403 => "Forbidden",
        502 => "Bad Gateway",
        504 => "Gateway Timeout",
        _ => "Error",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}\n",
        status, status_text, message
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_proxy_target_basic() {
        let target = parse_proxy_target("http://3.wispers.link/").unwrap();
        assert_eq!(target.node_number, 3);
        assert_eq!(target.port, 80);
        assert_eq!(target.path, "/");
    }

    #[test]
    fn test_parse_proxy_target_with_path() {
        let target = parse_proxy_target("http://42.wispers.link/api/v1/users").unwrap();
        assert_eq!(target.node_number, 42);
        assert_eq!(target.port, 80);
        assert_eq!(target.path, "/api/v1/users");
    }

    #[test]
    fn test_parse_proxy_target_with_port() {
        let target = parse_proxy_target("http://5.wispers.link:8080/test").unwrap();
        assert_eq!(target.node_number, 5);
        assert_eq!(target.port, 8080);
        assert_eq!(target.path, "/test");
    }

    #[test]
    fn test_parse_proxy_target_with_query() {
        let target = parse_proxy_target("http://1.wispers.link/search?q=test&page=2").unwrap();
        assert_eq!(target.node_number, 1);
        assert_eq!(target.port, 80);
        assert_eq!(target.path, "/search?q=test&page=2");
    }

    #[test]
    fn test_parse_proxy_target_no_path() {
        let target = parse_proxy_target("http://7.wispers.link").unwrap();
        assert_eq!(target.node_number, 7);
        assert_eq!(target.port, 80);
        assert_eq!(target.path, "/");
    }

    #[test]
    fn test_parse_proxy_target_invalid_no_http() {
        // HTTPS and relative paths should return 400 Bad Request
        let err = parse_proxy_target("https://3.wispers.link/").unwrap_err();
        assert_eq!(err.status_code(), 400);

        let err = parse_proxy_target("/path").unwrap_err();
        assert_eq!(err.status_code(), 400);
    }

    #[test]
    fn test_parse_proxy_target_forbidden_hostname() {
        // Non-wispers.link hosts should return 403 Forbidden
        let err = parse_proxy_target("http://example.com/").unwrap_err();
        assert_eq!(err.status_code(), 403);

        let err = parse_proxy_target("http://google.com/").unwrap_err();
        assert_eq!(err.status_code(), 403);
    }

    #[test]
    fn test_parse_proxy_target_invalid_node_number() {
        // Invalid node numbers in wispers.link should return 400
        let err = parse_proxy_target("http://abc.wispers.link/").unwrap_err();
        assert_eq!(err.status_code(), 400);

        let err = parse_proxy_target("http://0.wispers.link/").unwrap_err();
        assert_eq!(err.status_code(), 400);

        let err = parse_proxy_target("http://-1.wispers.link/").unwrap_err();
        assert_eq!(err.status_code(), 400);
    }

    #[test]
    fn test_hop_by_hop_headers() {
        assert!(is_hop_by_hop_header("connection"));
        assert!(is_hop_by_hop_header("keep-alive"));
        assert!(is_hop_by_hop_header("transfer-encoding"));
        assert!(!is_hop_by_hop_header("content-type"));
        assert!(!is_hop_by_hop_header("host"));
    }

    #[test]
    fn test_build_http_request() {
        let request = ParsedRequest {
            target: ProxyTarget {
                node_number: 3,
                port: 80,
                path: "/api/test".to_string(),
            },
            method: "GET".to_string(),
            version: 1,
            headers: vec![
                ("Host".to_string(), "3.wispers.link".to_string()),
                ("User-Agent".to_string(), "test/1.0".to_string()),
            ],
            keep_alive: true,
        };

        let http = build_http_request(&request);
        assert_eq!(
            http,
            "GET /api/test HTTP/1.1\r\nHost: 3.wispers.link\r\nUser-Agent: test/1.0\r\n\r\n"
        );
    }

    #[test]
    fn test_build_http_request_http10() {
        let request = ParsedRequest {
            target: ProxyTarget {
                node_number: 5,
                port: 8080,
                path: "/".to_string(),
            },
            method: "POST".to_string(),
            version: 0,
            headers: vec![("Host".to_string(), "5.wispers.link:8080".to_string())],
            keep_alive: false,
        };

        let http = build_http_request(&request);
        assert!(http.starts_with("POST / HTTP/1.0\r\n"));
    }
}
