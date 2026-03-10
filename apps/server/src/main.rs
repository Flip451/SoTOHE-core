// WARNING: This is a synchronous single-threaded stub server for template demonstration only.
// Connection handling runs serially on the main thread, causing head-of-line blocking.
// Before extending this for production use, replace with an async runtime (e.g. tokio + axum)
// or at minimum add thread-per-connection handling. See track/tech-stack.md for stack decisions.

use domain::new_user;
use infrastructure::InMemoryUserRepository;
use std::env;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::process::ExitCode;
use std::time::Duration;
use usecase::RegisterUserUseCase;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REQUEST_BYTES: usize = 8 * 1024;

fn main() -> ExitCode {
    let repo = InMemoryUserRepository::new();
    let usecase = RegisterUserUseCase::new(repo);
    let route = api::route_registration(&usecase);

    let user = match new_user("user-1") {
        Ok(user) => user,
        Err(err) => {
            eprintln!("failed to create bootstrap user: {err}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = usecase.execute(user) {
        eprintln!("failed to register bootstrap user: {err}");
        return ExitCode::FAILURE;
    }

    let addr = server_addr();
    let listener = match TcpListener::bind(&addr) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("failed to bind {addr}: {err}");
            return ExitCode::FAILURE;
        }
    };

    eprintln!("server listening on http://{addr}");
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("incoming connection failed: {err}");
                continue;
            }
        };

        if let Err(err) = stream.set_read_timeout(Some(READ_TIMEOUT)) {
            eprintln!("failed to configure read timeout: {err}");
            continue;
        }

        if let Err(err) = handle_connection_io(&mut stream, route) {
            eprintln!("connection handling failed: {err}");
        }
    }

    ExitCode::SUCCESS
}

fn server_addr() -> String {
    server_addr_from_port(env::var("PORT").ok().as_deref())
}

fn server_addr_from_port(port: Option<&str>) -> String {
    let port = port.unwrap_or("8080");
    format!("0.0.0.0:{port}")
}

fn handle_connection_io(
    stream: &mut (impl Read + Write),
    registration_route: &str,
) -> io::Result<()> {
    let request = match read_request(stream) {
        Ok(request) => request,
        Err(err) if matches!(err.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) => {
            let response = http_response(
                "408 Request Timeout",
                "text/plain; charset=utf-8",
                "request timeout\n",
            );
            stream.write_all(response.as_bytes())?;
            return stream.flush();
        }
        Err(err) => return Err(err),
    };
    let response = if find_header_end(request.as_bytes()).is_some() {
        build_response(&request, registration_route)
    } else {
        http_response("400 Bad Request", "text/plain; charset=utf-8", "bad request\n")
    };
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn read_request(reader: &mut impl Read) -> io::Result<String> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 512];

    loop {
        let read_len = reader.read(&mut buffer)?;
        if read_len == 0 {
            break;
        }

        let remaining = MAX_REQUEST_BYTES.saturating_sub(request.len());
        if remaining == 0 {
            break;
        }

        let copy_len = read_len.min(remaining);
        request.extend_from_slice(&buffer[..copy_len]);

        if let Some(header_end) = find_header_end(&request) {
            request.truncate(header_end);
            break;
        }

        if request.len() >= MAX_REQUEST_BYTES {
            break;
        }
    }

    Ok(String::from_utf8_lossy(&request).into_owned())
}

// Detects CRLF header terminator only (RFC 7230 §3). LF-only clients are not supported.
fn find_header_end(request: &[u8]) -> Option<usize> {
    request.windows(4).position(|window| window == b"\r\n\r\n").map(|position| position + 4)
}

fn build_response(request: &str, registration_route: &str) -> String {
    match parse_request_line(request) {
        Some(("GET", "/health")) => http_response("200 OK", "text/plain; charset=utf-8", "ok\n"),
        Some(("GET", "/")) => http_response(
            "200 OK",
            "text/plain; charset=utf-8",
            &format!("server running\nregistration route: {registration_route} (stub)\n"),
        ),
        Some(("GET", path)) if path == registration_route => {
            http_response("200 OK", "text/plain; charset=utf-8", "registration endpoint stub\n")
        }
        Some(_) => http_response("404 Not Found", "text/plain; charset=utf-8", "not found\n"),
        None => http_response("400 Bad Request", "text/plain; charset=utf-8", "bad request\n"),
    }
}

fn parse_request_line(request: &str) -> Option<(&str, &str)> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_REQUEST_BYTES, build_response, find_header_end, handle_connection_io,
        parse_request_line, read_request, server_addr_from_port,
    };
    use std::io::{self, Cursor, Read, Write};

    struct MockStream {
        read_cursor: Cursor<Vec<u8>>,
        read_error: Option<io::ErrorKind>,
        writes: Vec<u8>,
    }

    impl MockStream {
        fn from_bytes(bytes: &[u8]) -> Self {
            Self { read_cursor: Cursor::new(bytes.to_vec()), read_error: None, writes: Vec::new() }
        }

        fn with_read_error(kind: io::ErrorKind) -> Self {
            Self {
                read_cursor: Cursor::new(Vec::new()),
                read_error: Some(kind),
                writes: Vec::new(),
            }
        }

        fn written_string(&self) -> String {
            String::from_utf8(self.writes.clone()).unwrap()
        }
    }

    impl Read for MockStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if let Some(kind) = self.read_error.take() {
                return Err(io::Error::new(kind, "mock read error"));
            }
            self.read_cursor.read(buf)
        }
    }

    impl Write for MockStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_parse_request_line_extracts_method_and_path() {
        let request = "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(parse_request_line(request), Some(("GET", "/health")));
    }

    #[test]
    fn test_build_response_returns_health_check_body() {
        let response = build_response("GET /health HTTP/1.1\r\n\r\n", "/register");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("ok\n"));
    }

    #[test]
    fn test_build_response_returns_root_description() {
        let response = build_response("GET / HTTP/1.1\r\n\r\n", "/register");
        assert!(response.contains("registration route: /register (stub)\n"));
    }

    #[test]
    fn test_build_response_returns_registration_stub() {
        let response = build_response("GET /register HTTP/1.1\r\n\r\n", "/register");
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("registration endpoint stub\n"));
    }

    #[test]
    fn test_build_response_rejects_unknown_path() {
        let response = build_response("GET /missing HTTP/1.1\r\n\r\n", "/register");
        assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
    }

    #[test]
    fn test_build_response_rejects_malformed_request() {
        let response = build_response("BROKEN\r\n\r\n", "/register");
        assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    }

    #[test]
    fn test_server_addr_uses_default_port() {
        assert_eq!(server_addr_from_port(None), "0.0.0.0:8080");
    }

    #[test]
    fn test_server_addr_uses_env_port_when_present() {
        assert_eq!(server_addr_from_port(Some("9090")), "0.0.0.0:9090");
    }

    #[test]
    fn test_read_request_reads_until_header_terminator() {
        let mut reader =
            Cursor::new(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\nignored body");

        let request = read_request(&mut reader).unwrap();

        assert!(request.ends_with("\r\n\r\n"));
        assert!(!request.contains("ignored body"));
    }

    #[test]
    fn test_read_request_caps_buffer_when_headers_do_not_terminate() {
        let payload = vec![b'a'; MAX_REQUEST_BYTES + 128];
        let mut reader = Cursor::new(payload);

        let request = read_request(&mut reader).unwrap();

        assert_eq!(request.len(), MAX_REQUEST_BYTES);
    }

    #[test]
    fn test_read_request_returns_partial_headers_when_stream_ends_early() {
        let mut reader = Cursor::new(b"GET /health HTTP/1.1\r\nHost: localhost\r\n");

        let request = read_request(&mut reader).unwrap();

        assert_eq!(request, "GET /health HTTP/1.1\r\nHost: localhost\r\n");
    }

    #[test]
    fn test_find_header_end_returns_end_of_header_bytes() {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\nbody";

        assert_eq!(find_header_end(request), Some(35));
    }

    #[test]
    fn test_handle_connection_returns_timeout_response() {
        let mut stream = MockStream::with_read_error(io::ErrorKind::TimedOut);

        handle_connection_io(&mut stream, "/register").unwrap();

        let response = stream.written_string();

        assert!(response.starts_with("HTTP/1.1 408 Request Timeout\r\n"));
        assert!(response.ends_with("request timeout\n"));
    }

    #[test]
    fn test_handle_connection_returns_not_found_for_unknown_route() {
        let mut stream =
            MockStream::from_bytes(b"GET /missing HTTP/1.1\r\nHost: localhost\r\n\r\n");

        handle_connection_io(&mut stream, "/register").unwrap();

        let response = stream.written_string();

        assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(response.ends_with("not found\n"));
    }

    #[test]
    fn test_handle_connection_returns_registration_stub() {
        let mut stream =
            MockStream::from_bytes(b"GET /register HTTP/1.1\r\nHost: localhost\r\n\r\n");

        handle_connection_io(&mut stream, "/register").unwrap();

        let response = stream.written_string();

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("registration endpoint stub\n"));
    }

    #[test]
    fn test_handle_connection_rejects_partial_headers_as_bad_request() {
        let mut stream = MockStream::from_bytes(b"GET /health HTTP/1.1\r\nHost: localhost\r\n");

        handle_connection_io(&mut stream, "/register").unwrap();

        let response = stream.written_string();

        assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
        assert!(response.ends_with("bad request\n"));
    }
}
