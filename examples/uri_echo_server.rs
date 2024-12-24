use std::{io::Cursor, net::TcpListener};

use streetlight::{header, read_request, write_response, Response, StatusCode};

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    for stream in listener.incoming() {
        if let Ok(mut s) = stream {
            let request = match read_request(&mut s) {
                Ok(r) => r,
                Err(e) => {
                    println!("{}", e);
                    continue
                }
            };

            let response_body = format!("<h1>You requested {}</h1>", request.uri());

            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, response_body.len())
                .header(header::CONTENT_TYPE, "text/html")
                .body(Cursor::new(response_body))
                .unwrap();

            write_response(&mut s, response)?;
        }
    }

    Ok(())
}
