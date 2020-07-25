#![feature(proc_macro_hygiene, decl_macro)]

mod autocomplete;
mod streamed_string_list_response;
use streamed_string_list_response::StreamedStringListResponse;

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate lazy_static;

#[get("/")]
fn hello() -> &'static str {
    "Hello, world!"
}

#[get("/<project>/autocomplete?<q>&<count>")]
fn autocomplete_view(
    project: String,
    q: Option<String>,
    count: Option<usize>,
) -> Option<StreamedStringListResponse> {
    Some(StreamedStringListResponse::new(autocomplete::complete(
        project,
        q.unwrap_or_default(),
        count.unwrap_or(50),
    )))
}

fn main() {
    autocomplete::init();
    rocket::ignite()
        .mount("/", routes![hello, autocomplete_view])
        .launch();
}
