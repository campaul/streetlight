use std::env;
use std::{io::Cursor, net::TcpStream};

use streetlight::{read_response, write_request, Method, Request};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let url = &args[1];

    let mut stream = TcpStream::connect(url)?;

    let request = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Cursor::new(vec![]))
        .unwrap();

    write_request(&mut stream, request)?;

    let response = read_response(&mut stream)?;

    println!("Status code: {}", response.status());

    Ok(())
}
