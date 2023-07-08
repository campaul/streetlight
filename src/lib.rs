use std::io::{prelude::*, BufReader, Cursor};

pub use http::{header, Method, Request, Response, StatusCode, Version};

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

pub fn read_request(reader: &mut dyn Read) -> std::io::Result<Request<impl Read>> {
    let mut buf_reader = BufReader::new(reader);
    let mut request = Request::builder();

    let mut start_line = String::new();
    buf_reader.read_line(&mut start_line)?;

    let start_line_fields = start_line.trim().split(" ").collect::<Vec<&str>>();

    if start_line_fields.len() != 3 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Unable to parse start line",
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

        match line.split_once(":") {
            Some((name, value)) => {
                let value = value.trim();

                if let Err(_) = header::HeaderName::try_from(name) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Invalid header name: {}", name),
                    ));
                }

                if let Err(_) = header::HeaderValue::try_from(value) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Invalid header value: {}", value),
                    ));
                }

                request = request.header(name, value);

                if name == header::CONTENT_LENGTH {
                    content_length = value.parse::<u64>().unwrap();
                }
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Unable to parse header line: {}", line),
                ))
            }
        }
    }

    let mut body: Vec<u8> = vec![0; content_length as usize];

    if content_length > 0 {
        buf_reader.read_exact(&mut body)?;
    }

    match request.body(Cursor::new(body)) {
        Ok(r) => Ok(r),
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
    }
}

pub fn read_response(reader: &mut dyn Read) -> std::io::Result<Response<impl Read>> {
    let mut buf_reader = BufReader::new(reader);
    let mut response = Response::builder();

    let mut start_line = String::new();
    buf_reader.read_line(&mut start_line)?;

    let start_line_fields = start_line.trim().split(" ").collect::<Vec<&str>>();

    if start_line_fields.len() != 2 && start_line_fields.len() != 3 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Unable to parse start line",
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

        match line.split_once(":") {
            Some((name, value)) => {
                let value = value.trim();

                if let Err(_) = header::HeaderName::try_from(name) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Invalid header name: {}", name),
                    ));
                }

                if let Err(_) = header::HeaderValue::try_from(value) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Invalid header value: {}", value),
                    ));
                }

                response = response.header(name, value);

                if name == header::CONTENT_LENGTH {
                    content_length = value.parse::<u64>().unwrap();
                }
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Unable to parse header line: {}", line),
                ))
            }
        }
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
    writer: &mut dyn Write,
    mut request: Request<impl Read>,
) -> std::io::Result<()> {
    let method = request.method();
    let uri = request.uri();
    let version = format!("{:?}", request.version());
    let start_line = format!("{} {} {}\r\n", method, uri, version);

    writer.write_all(start_line.as_bytes())?;

    for (header_name, header_value) in request.headers().iter() {
        if let Ok(value) = header_value.to_str() {
            writer.write(format!("{}: {}\r\n", header_name, value).as_bytes())?;
        }
    }

    writer.write_all("\r\n".as_bytes())?;

    match request.headers().get(header::CONTENT_LENGTH) {
        Some(v) => {
            let content_length = v.to_str().unwrap().parse::<u32>().unwrap();
            let mut body: Vec<u8> = vec![0; content_length as usize];

            let mut buf_reader = BufReader::new(request.body_mut());
            if content_length > 0 {
                buf_reader.read_exact(&mut body)?;
            }

            writer.write_all(&body)?;
        }
        None => {}
    }

    Ok(())
}

pub fn write_response(
    writer: &mut dyn Write,
    mut response: Response<impl Read>,
) -> std::io::Result<()> {
    let version = format!("{:?}", response.version());
    let status = response.status();
    let start_line = format!("{} {}\r\n", version, status);

    writer.write_all(start_line.as_bytes())?;

    for (header_name, header_value) in response.headers().iter() {
        if let Ok(value) = header_value.to_str() {
            writer.write(format!("{}: {}\r\n", header_name, value).as_bytes())?;
        }
    }

    writer.write_all("\r\n".as_bytes())?;

    match response.headers().get(header::CONTENT_LENGTH) {
        Some(v) => {
            let content_length = v.to_str().unwrap().parse::<u32>().unwrap();
            let mut body: Vec<u8> = vec![0; content_length as usize];

            let mut buf_reader = BufReader::new(response.body_mut());
            if content_length > 0 {
                buf_reader.read_exact(&mut body)?;
            }

            writer.write_all(&body)?;
        }
        None => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str;

    use super::*;

    #[test]
    fn read_simple_request() {
        let request_text = "GET / HTTP/1.1\r\n\
                            host: example.com\r\n\
                            \r\n";

        let request = read_request(&mut Cursor::new(request_text.as_bytes())).unwrap();

        assert!(request.method() == "GET");
        assert!(request.uri() == "/");
        assert!(request.version() == Version::HTTP_11);

        assert!(request.headers().get(header::HOST).unwrap() == "example.com");
    }

    #[test]
    fn read_simple_response() {
        let response_text = "HTTP/1.1 200 OK\r\n\
                            content-type: text/html\r\n\
                            content-length: 20\r\n\
                            \r\n\
                            <h1>Hello World</h1>";

        let mut response = read_response(&mut Cursor::new(response_text.as_bytes())).unwrap();

        assert!(response.version() == Version::HTTP_11);
        assert!(response.status() == StatusCode::OK);
        assert!(response.status().canonical_reason().unwrap() == "OK");

        assert!(response.headers().get(header::CONTENT_TYPE).unwrap() == "text/html");
        assert!(response.headers().get(header::CONTENT_LENGTH).unwrap() == "20");

        let mut body_buffer: Vec<u8> = vec![0; 20];
        response.body_mut().read_exact(&mut body_buffer).unwrap();

        assert!(str::from_utf8(&body_buffer).unwrap() == "<h1>Hello World</h1>");
    }

    #[test]
    fn write_simple_request() {
        let body: Cursor<Vec<u8>> = Cursor::new(vec![]);

        let request = Request::builder()
            .method("GET")
            .uri("/")
            .version(Version::HTTP_11)
            .header(header::HOST, "example.com")
            .body(body)
            .unwrap();

        let mut request_bytes: Vec<u8> = vec![];
        write_request(&mut request_bytes, request).unwrap();

        let request_text = "GET / HTTP/1.1\r\n\
                            host: example.com\r\n\
                            \r\n";

        assert!(str::from_utf8(&request_bytes).unwrap() == request_text);
    }

    #[test]
    fn write_simple_response() {
        let body: Cursor<Vec<u8>> = Cursor::new("<h1>Hello World</h1>".as_bytes().to_vec());

        let response = Response::builder()
            .version(Version::HTTP_11)
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CONTENT_LENGTH, 20)
            .body(body)
            .unwrap();

        let mut response_bytes: Vec<u8> = vec![];
        write_response(&mut response_bytes, response).unwrap();

        let response_text = "HTTP/1.1 200 OK\r\n\
                            content-type: text/html\r\n\
                            content-length: 20\r\n\
                            \r\n\
                            <h1>Hello World</h1>";

        assert!(str::from_utf8(&response_bytes).unwrap() == response_text);
    }
}
