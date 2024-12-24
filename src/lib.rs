use std::io::{prelude::*, BufReader, Cursor};
use std::net::TcpStream;

pub use http::{header, Method, Request, Response, StatusCode, Uri, Version};
use http::{HeaderName, HeaderValue};

fn parse_version(version: &str) -> std::io::Result<Version> {
    match version {
        "HTTP/0.9" => Ok(Version::HTTP_09),
        "HTTP/1.0" => Ok(Version::HTTP_10),
        "HTTP/1.1" => Ok(Version::HTTP_11),
        "HTTP/2.0" => Ok(Version::HTTP_2),
        "HTTP/3.0" => Ok(Version::HTTP_3),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Invalid HTTP version: {}", version),
        )),
    }
}

// TODO: it's not correct to assume a header line is a valid unicode string
// This needs to be updated to use bytes instead
fn parse_header(line: &str) -> std::io::Result<(HeaderName, HeaderValue)> {
    match line.split_once(":") {
        Some((name, value)) => {
            let value = value.trim();

            let n = match header::HeaderName::try_from(name) {
                Ok(n) => n,
                Err(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Invalid header name: {}", name),
                    ));
                }
            };

            let v = match header::HeaderValue::try_from(value) {
                Ok(v) => v,
                Err(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Invalid header value: {}", value),
                    ));
                }
            };

            Ok((n, v))
        }
        None => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Unable to parse header line: {}", line),
        )),
    }
}

pub fn read_request(tcp_stream: &mut TcpStream) -> std::io::Result<Request<impl Read>> {
    let mut buf_reader = BufReader::new(tcp_stream);
    let mut request = Request::builder();

    let mut start_line = String::new();
    buf_reader.read_line(&mut start_line)?;

    if start_line.len() == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Connection Closed",
        ));
    }

    let start_line_fields = start_line.trim().split(" ").collect::<Vec<&str>>();

    if start_line_fields.len() != 3 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Unable to parse start line: {}", start_line),
        ));
    }

    let method = start_line_fields[0];
    let uri = start_line_fields[1];
    let version = start_line_fields[2];

    request = request.method(method);
    request = request.uri(uri);
    request = request.version(parse_version(version)?);

    let mut content_length = 0;

    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line)?;

        if line == "\r\n" {
            break;
        }

        let (name, value) = parse_header(&line)?;

        if name == header::CONTENT_LENGTH {
            content_length = value.to_str().unwrap().parse::<usize>().unwrap();
        }

        request = request.header(name, value);
    }

    let mut body: Vec<u8> = vec![0; content_length];

    if content_length > 0 {
        buf_reader.read_exact(&mut body)?;
    }

    match request.body(Cursor::new(body)) {
        Ok(r) => Ok(r),
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
    }
}

pub fn read_response(tcp_stream: &mut TcpStream) -> std::io::Result<Response<impl Read>> {
    let mut buf_reader = BufReader::new(tcp_stream);
    let mut response = Response::builder();

    let mut start_line = String::new();
    buf_reader.read_line(&mut start_line)?;

    let start_line_fields = start_line.trim().split(" ").collect::<Vec<&str>>();

    if start_line_fields.len() != 2 && start_line_fields.len() != 3 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Unable to parse start line: {}", start_line),
        ));
    }

    let version = start_line_fields[0];
    let status = start_line_fields[1];

    response = response.version(parse_version(version)?);
    response = response.status(status);

    let mut content_length = 0;

    loop {
        let mut line = String::new();
        buf_reader.read_line(&mut line)?;

        if line == "\r\n" {
            break;
        }

        let (name, value) = parse_header(&line)?;

        if name == header::CONTENT_LENGTH {
            content_length = value.to_str().unwrap().parse::<usize>().unwrap();
        }

        response = response.header(name, value);
    }

    let mut body: Vec<u8> = vec![0; content_length as usize];

    if content_length > 0 {
        buf_reader.read_exact(&mut body)?;
    }

    match response.body(Cursor::new(body)) {
        Ok(r) => Ok(r),
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
    }
}

pub fn write_request(
    tcp_stream: &mut TcpStream,
    mut request: Request<impl Read>,
) -> std::io::Result<()> {
    let method = request.method();
    let uri = request.uri();
    let version = format!("{:?}", request.version());
    let start_line = format!("{} {} {}\r\n", method, uri, version);

    tcp_stream.write_all(start_line.as_bytes())?;

    for (header_name, header_value) in request.headers().iter() {
        if let Ok(value) = header_value.to_str() {
            tcp_stream.write(format!("{}: {}\r\n", header_name, value).as_bytes())?;
        }
    }

    tcp_stream.write_all("\r\n".as_bytes())?;

    match request.headers().get(header::CONTENT_LENGTH) {
        Some(v) => {
            let content_length = v.to_str().unwrap().parse::<u32>().unwrap();
            let mut body: Vec<u8> = vec![0; content_length as usize];

            let mut buf_reader = BufReader::new(request.body_mut());
            if content_length > 0 {
                buf_reader.read_exact(&mut body)?;
            }

            tcp_stream.write_all(&body)?;
        }
        None => {}
    }

    Ok(())
}

pub fn write_response(
    tcp_stream: &mut TcpStream,
    mut response: Response<impl Read>,
) -> std::io::Result<()> {
    let version = format!("{:?}", response.version());
    let status = response.status();
    let start_line = format!("{} {}\r\n", version, status);

    tcp_stream.write_all(start_line.as_bytes())?;

    for (header_name, header_value) in response.headers().iter() {
        if let Ok(value) = header_value.to_str() {
            tcp_stream.write(format!("{}: {}\r\n", header_name, value).as_bytes())?;
        }
    }

    tcp_stream.write_all("\r\n".as_bytes())?;

    match response.headers().get(header::CONTENT_LENGTH) {
        Some(v) => {
            let content_length = v.to_str().unwrap().parse::<u32>().unwrap();
            let mut body: Vec<u8> = vec![0; content_length as usize];

            let mut buf_reader = BufReader::new(response.body_mut());
            if content_length > 0 {
                buf_reader.read_exact(&mut body)?;
            }

            tcp_stream.write_all(&body)?;
        }
        None => {}
    }

    Ok(())
}

pub fn read_chunk(tcp_stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buf_reader = BufReader::new(tcp_stream);

    let mut len = String::new();
    buf_reader.read_line(&mut len)?;

    match len.trim().parse::<usize>() {
        Ok(len) => {
            let mut chunk: Vec<u8> = vec![0; len];

            buf_reader.read_exact(&mut chunk)?;

            let mut tail = String::new();
            buf_reader.read_line(&mut tail)?;

            Ok(chunk)
        }
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Invalid chunk size",
        )),
    }
}

pub fn write_chunk(tcp_stream: &mut TcpStream, chunk: &[u8]) -> std::io::Result<()> {
    tcp_stream.write_all(format!("{}\r\n", chunk.len()).as_bytes())?;
    tcp_stream.write_all(chunk)?;
    tcp_stream.write_all("\r\n".as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_simple_request() {
        let request_text = "GET / HTTP/1.1\r\n\
                            host: example.com\r\n\
                            \r\n";

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut tcp_stream = std::net::TcpStream::connect(addr).unwrap();
        tcp_stream.write(request_text.as_bytes()).unwrap();

        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                let request = read_request(&mut s).unwrap();

                assert!(request.method() == "GET");
                assert!(request.uri() == "/");
                assert!(request.version() == Version::HTTP_11);

                assert!(request.headers().get(header::HOST).unwrap() == "example.com");

                break;
            }
        }
    }

    #[test]
    fn read_simple_response() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut tcp_stream = std::net::TcpStream::connect(addr).unwrap();

        let response_text = "HTTP/1.1 200 OK\r\n\
                            content-type: text/html\r\n\
                            content-length: 20\r\n\
                            \r\n\
                            <h1>Hello World</h1>";

        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                s.write(response_text.as_bytes()).unwrap();

                break;
            }
        }

        let mut response = read_response(&mut tcp_stream).unwrap();

        assert!(response.version() == Version::HTTP_11);
        assert!(response.status() == StatusCode::OK);
        assert!(response.status().canonical_reason().unwrap() == "OK");

        assert!(response.headers().get(header::CONTENT_TYPE).unwrap() == "text/html");
        assert!(response.headers().get(header::CONTENT_LENGTH).unwrap() == "20");

        let mut body_buffer: Vec<u8> = vec![0; 20];
        response.body_mut().read_exact(&mut body_buffer).unwrap();

        assert!(std::str::from_utf8(&body_buffer).unwrap() == "<h1>Hello World</h1>");
    }

    #[test]
    fn write_simple_request() {
        let body: Cursor<Vec<u8>> = Cursor::new(vec![]);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut tcp_stream = std::net::TcpStream::connect(addr).unwrap();

        let request = Request::builder()
            .method("GET")
            .uri("/")
            .version(Version::HTTP_11)
            .header(header::HOST, "example.com")
            .body(body)
            .unwrap();

        write_request(&mut tcp_stream, request).unwrap();

        let request_text = "GET / HTTP/1.1\r\n\
                            host: example.com\r\n\
                            \r\n";

        for stream in listener.incoming() {
            if let Ok(s) = stream {
                let mut buf_reader = BufReader::new(s);
                let mut buffer = String::new();

                for _ in 0..request_text.lines().count() {
                    buf_reader.read_line(&mut buffer).unwrap();
                }

                assert!(buffer.as_str() == request_text);

                break;
            }
        }
    }

    #[test]
    fn write_simple_response() {
        let body: Cursor<Vec<u8>> = Cursor::new("<h1>Hello World</h1>".as_bytes().to_vec());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let tcp_stream = std::net::TcpStream::connect(addr).unwrap();

        let response = Response::builder()
            .version(Version::HTTP_11)
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CONTENT_LENGTH, 20)
            .body(body)
            .unwrap();

        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                write_response(&mut s, response).unwrap();

                break;
            }
        }

        let response_text = "HTTP/1.1 200 OK\r\n\
                            content-type: text/html\r\n\
                            content-length: 20\r\n\
                            \r\n\
                            <h1>Hello World</h1>";

        let mut buf_reader = BufReader::new(tcp_stream);
        let mut buffer = String::new();

        for _ in 0..response_text.lines().count() {
            buf_reader.read_line(&mut buffer).unwrap();
        }

        assert!(buffer.as_str() == response_text);
    }
}
