use std::{
    env,
    fs::{self, File},
    io::{Cursor, Write},
    net::TcpListener,
    path::Path,
};

use streetlight::{header, read_request, write_response, Response, StatusCode, Uri};

fn write_not_found(w: &mut impl Write) -> std::io::Result<()> {
    let response = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Cursor::new(vec![]))
        .unwrap();

    write_response(w, response)
}

fn safe_file_path(uri: &Uri) -> std::io::Result<Box<Path>> {
    let relative_path = format!("./{}", uri);
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

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    for stream in listener.incoming() {
        if let Ok(mut s) = stream {
            let request = read_request(&mut s)?;

            if let Ok(path) = safe_file_path(request.uri()) {
                if let Ok(file) = File::open(path) {
                    if !file.metadata()?.is_file() {
                        write_not_found(&mut s)?;
                        continue;
                    }

                    let response = Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_LENGTH, file.metadata()?.len())
                        .header(header::CONTENT_TYPE, "text/plain")
                        .body(file)
                        .unwrap();

                    write_response(&mut s, response)?;
                }
            }

            write_not_found(&mut s)?;
        }
    }

    Ok(())
}
