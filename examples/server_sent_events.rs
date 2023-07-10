use std::{
    io::{Cursor, Write},
    net::TcpListener,
    thread, time,
};

use http::header;
use streetlight::{read_request, write_chunk, write_response, Response, StatusCode};

const PAGE: &str = "\
    <html>\
        <body>\
            <h1></h1>
            <script>\
                const eventSource = new EventSource('/events');\
                eventSource.addEventListener('count', (event) => {\
                    document.querySelector('h1').innerText = JSON.parse(event.data);\
                });\
            </script>\
        </body>\
    </html>";

fn respond_page(stream: &mut dyn Write) -> std::io::Result<()> {
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::CONTENT_LENGTH, PAGE.len())
        .body(Cursor::new(PAGE.as_bytes()))
        .unwrap();

    write_response(stream, response)?;

    Ok(())
}

fn respond_stream(stream: &mut dyn Write) -> std::io::Result<()> {
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Cursor::new(vec![]))
        .unwrap();

    write_response(stream, response)?;

    let mut i = 0;

    loop {
        write_chunk(stream, format!("event: count\ndata: {}\n\n", i).as_bytes()).unwrap();

        i += 1;

        thread::sleep(time::Duration::from_millis(1000));
    }
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    for stream in listener.incoming() {
        if let Ok(mut s) = stream {
            let request = read_request(&mut s)?;

            if request.uri() == "/" {
                respond_page(&mut s)?;
            } else {
                respond_stream(&mut s)?;
            }
        }
    }

    Ok(())
}
