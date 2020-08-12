use rocket::http::ContentType;
use rocket::response::Responder;
use rocket::{response, Request, Response};
use std::collections::VecDeque;
use std::io::Read;
use std::mem;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

pub struct StreamedStringListResponse {
    source: Receiver<String>,
    buffer: VecDeque<u8>,
    start: Instant,
    counter: u64,
}

// Source: https://github.com/SergioBenitez/Rocket/issues/1298#issuecomment-629516104
impl Read for StreamedStringListResponse {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        while self.buffer.len() < 4096 {
            match self.source.try_recv() {
                Ok(chunk) => chunk
                    .as_bytes()
                    .iter()
                    .for_each(|c| self.buffer.push_back(*c)),
                Err(TryRecvError::Empty) => {
                    if self.buffer.len() == 0 {
                        if let Ok(chunk) = self.source.recv() {
                            for c in chunk.as_bytes().iter() {
                                self.buffer.push_back(*c);
                            }
                        } else {
                            break;
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    break;
                }
            }
        }
        // If we reached here and the buffer's empty - then the channel is closed.
        if self.buffer.len() == 0 {
            println!("streamed {} bytes in {:?}", self.counter, self.start.elapsed());
            return Ok(0);
        }

        // Split off a chunk from the beginning
        let remaining = self.buffer.split_off(buf.len().min(self.buffer.len()));
        let chunk = mem::replace(&mut self.buffer, remaining);
        let (slice1, slice2) = chunk.as_slices();

        // And put it into `buf`
        for (src, dst) in slice1.iter().chain(slice2.iter()).zip(buf.iter_mut()) {
            *dst = *src;
        }
        self.counter += (slice1.len() + slice2.len()) as u64;
        Ok(slice1.len() + slice2.len())
    }
}

impl StreamedStringListResponse {
    pub fn new(source: Receiver<String>) -> Self {
        StreamedStringListResponse {
            source,
            buffer: VecDeque::with_capacity(4096),
            start: Instant::now(),
            counter: 0,
        }
    }
}

impl<'r> Responder<'r> for StreamedStringListResponse {
    fn respond_to(self, _: &Request) -> response::Result<'r> {
        Response::build()
            .header(ContentType::HTML)
            .raw_header("Cache-Control", "max-age=604800") // 7*24*60*60
            .streamed_body(self)
            .ok()
    }
}
