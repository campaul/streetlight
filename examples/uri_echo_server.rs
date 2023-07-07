use std::{io::Cursor, net::TcpListener};

use streetlight::{read_request, write_response, Response};

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    for stream in listener.incoming() {
        if let Ok(mut s) = stream {
            let request = read_request(&mut s)?;

            let response_body = format!("<h1>You requested {}</h1>", request.uri());

            let response = Response::builder()
                .header("content-length", response_body.len())
                .header("content-type", "text/html")
                .body(Cursor::new(response_body))
                .unwrap();

            write_response(&mut s, response)?;
        }
    }

    Ok(())
}
