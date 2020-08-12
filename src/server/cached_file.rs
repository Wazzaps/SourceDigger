use rocket::response::{Responder};
use rocket::{Response, Request, response};

pub(crate) struct CachedFile<R: Responder<'static>>(pub(crate) R);

impl<R: Responder<'static>> Responder<'static> for CachedFile<R> {
    fn respond_to(self, req: &Request) -> response::Result<'static> {
        Response::build_from(self.0.respond_to(req)?)
            .raw_header("Cache-control", "max-age=86400") //  24h (24*60*60)
            .ok()
    }
}