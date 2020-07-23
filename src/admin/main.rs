#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

mod routes;
#[path="../data.rs"]
mod data;

use rocket_contrib::serve::StaticFiles;
use rocket_contrib::templates::Template;

fn main() {
    rocket::ignite()
        .attach(Template::fairing())
        .mount(
            "/",
            routes![
                routes::admin_index,
                routes::admin_add_repo,
                routes::admin_add_repo_post,
                routes::admin_view_repo
            ],
        )
        .mount("/static", StaticFiles::from("./static"))
        .launch();
}
