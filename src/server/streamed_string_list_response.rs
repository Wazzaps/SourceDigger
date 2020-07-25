use rocket::response::Responder;
use rocket::{Request, response, Response};
use std::io::{Cursor, Read};
use std::sync::mpsc::Receiver;
use rocket::http::ContentType;

pub struct StreamedStringListResponse {
    source: Receiver<String>,
    cursor: Option<Cursor<String>>,
}

// Source: https://github.com/SergioBenitez/Rocket/issues/1298#issuecomment-629516104
impl Read for StreamedStringListResponse {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        // println!("read");
        if self.cursor.is_none() {
            // Get the next chunk
            let chunk = match self.source.iter().next() {
                Some(f) => f,
                None => {
                    // println!("Done");
                    return Ok(0);
                },
            };

            self.cursor = Some(Cursor::new(chunk));
        }

        // Read the current chunk into the buffer
        let cursor = self.cursor.as_mut().unwrap();
        let n = cursor.read(buf);
        // If we completed the chunk, reset `cursor` to `None` so that
        // the next chunk will be taken the next time `read` is called
        if cursor.position() == cursor.get_ref().len() as u64 {
            self.cursor = None;
        }
        if n.is_err() {
            println!("err");
        }
        let n = n.unwrap();
        // println!("returning {} bytes", n);
        Ok(n)
    }
}

impl StreamedStringListResponse {
    pub fn new(source: Receiver<String>) -> Self {
        StreamedStringListResponse {
            source,
            cursor: None,
        }
    }
}

impl<'r> Responder<'r> for StreamedStringListResponse {
    fn respond_to(self, _: &Request) -> response::Result<'r> {
        Response::build()
            .header(ContentType::Plain)
            .streamed_body(self)
            .ok()
    }
}