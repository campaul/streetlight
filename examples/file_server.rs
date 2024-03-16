use std::{
    env,
    ffi::OsStr,
    fs::{self, File},
    io::{BufReader, Cursor, Read, Write},
    net::TcpListener,
    path::Path,
};

use flate2::read::GzEncoder;
use flate2::Compression;

use streetlight::{header, read_request, write_response, Response, StatusCode, Uri};

fn log_and_write_response<T: std::io::Read>(
    w: &mut impl Write,
    response: Response<T>,
    filename: String,
) -> std::io::Result<()> {
    println!("{} {}", response.status(), filename);
    write_response(w, response)
}

fn write_not_found(w: &mut impl Write, filename: String) -> std::io::Result<()> {
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_LENGTH, 0)
        .body(Cursor::new(vec![]))
        .unwrap();

    log_and_write_response(w, response, filename)
}

fn safe_file_path(uri: &Uri) -> std::io::Result<Box<Path>> {
    let relative_path = match uri.path() {
        "/" => String::from("./index.html"),
        _ => format!("./{}", uri),
    };

    let canonical_path = fs::canonicalize(relative_path)?.into_boxed_path();

    let pwd = env::current_dir()?.into_boxed_path();

    if canonical_path.starts_with(pwd) {
        return Ok(canonical_path);
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Invalid path",
    ))
}

fn handle_request(s: &mut std::net::TcpStream) -> std::io::Result<()> {
    let request = read_request(s)?;
    let filename = format!("{}", request.uri());

    match safe_file_path(request.uri()) {
        Ok(path) => {
            let content_type = match path.extension().and_then(OsStr::to_str) {
                Some("html") => "text/html",
                Some("css") => "text/css",
                Some("js") => "text/javascript",
                _ => "text/plain",
            };

            if let Ok(file) = File::open(path.clone()) {
                if !file.metadata()?.is_file() {
                    write_not_found(s, filename)?;
                    return Ok(());
                }

                let mut gz = GzEncoder::new(BufReader::new(file), Compression::default());
                let mut buffer = Vec::new();
                gz.read_to_end(&mut buffer)?;

                let response = Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_LENGTH, buffer.len())
                    .header(header::CONTENT_TYPE, content_type)
                    .header(header::CONTENT_ENCODING, "gzip")
                    .body(buffer.as_slice())
                    .unwrap();

                log_and_write_response(s, response, filename)?;
            }
        }
        Err(_) => write_not_found(s, filename)?,
    }

    Ok(())
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;

    println!("Listening @ 127.0.0.1:8080\n");

    for stream in listener.incoming() {
        if let Ok(mut s) = stream {
            // Ignore the browser dropping the connection
            let _ = handle_request(&mut s);
        }
    }

    Ok(())
}
