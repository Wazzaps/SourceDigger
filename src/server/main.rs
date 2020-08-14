#![feature(proc_macro_hygiene, decl_macro)]

mod autocomplete;
mod diffs;
mod streamed_string_list_response;
mod cached_file;
use cached_file::CachedFile;
#[path="../data.rs"]
mod data;

use inflector::Inflector;
use rocket_contrib::serve::StaticFiles;
use rocket_contrib::templates::Template;
use std::collections::HashMap;
use streamed_string_list_response::StreamedStringListResponse;
use url::form_urlencoded;
use rocket::response::NamedFile;
use std::path::{Path, PathBuf};

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate lazy_static;

#[get("/<project>/autocomplete?<q>&<count>")]
fn autocomplete_view(
    project: String,
    q: Option<String>,
    count: Option<u64>,
) -> CachedFile<StreamedStringListResponse> {
    CachedFile(StreamedStringListResponse::new(autocomplete::complete(
        project,
        q.unwrap_or(".*".into()),
        count.unwrap_or(100),
    )))
}

#[get("/<project>/diffs?<q>&<a>&<t>&<count>")]
fn search_view(
    project: String,
    q: String,
    a: Option<String>,
    t: Option<String>,
    count: Option<u64>,
) -> CachedFile<StreamedStringListResponse> {
    CachedFile(StreamedStringListResponse::new(diffs::get_diffs(
        project,
        q,
        a.unwrap_or("arm".into()),
        t.unwrap_or("fvd".into()),
        count.unwrap_or(u64::max_value()),
    )))
}

#[get("/<_project>/diffs")]
fn empty_search_view(
    _project: String,
) -> CachedFile<Template> {
    let context = HashMap::<String, String>::new();
    CachedFile(Template::render("welcome", &context))
}

#[get("/<project>?<q>&<a>&<t>")]
fn project_view(project: String, q: Option<String>, a: Option<String>, t: Option<String>) -> CachedFile<Template> {
    let mut context = HashMap::<String, String>::new();
    context.insert("project".into(), project.clone());
    context.insert("Project".into(), project.to_title_case());

    let mut diff_params = form_urlencoded::Serializer::new(String::new());
    if let Some(q) = &q {
        diff_params.append_pair("q", q);
    }
    if let Some(a) = &a {
        diff_params.append_pair("a", a);
    }
    if let Some(t) = &t {
        diff_params.append_pair("t", t);
    }

    context.insert("diff_params".into(), diff_params.finish());
    context.insert("q".into(), q.clone().unwrap_or("".into()));
    context.insert(
        "query_dash".into(),
        q.map(|s| s + " - ").unwrap_or("".into()),
    );

    CachedFile(Template::render("results", &context))
}

#[get("/<project>/project_logo.png")]
fn project_logo_view(project: String) -> Option<CachedFile<NamedFile>> {
    NamedFile::open(Path::new("sourcedigger-db").join(project).join("logo.png")).ok().map(|nf| CachedFile(nf))
}

#[get("/<project>/project_logo.svg")]
fn project_logo_svg_view(project: String) -> Option<CachedFile<NamedFile>> {
    NamedFile::open(Path::new("sourcedigger-db").join(project).join("logo.svg")).ok().map(|nf| CachedFile(nf))
}

#[get("/robots.txt")]
fn robots_txt_view() -> Option<CachedFile<NamedFile>> {
    NamedFile::open("static/robots.txt").ok().map(|nf| CachedFile(nf))
}

#[get("/")]
fn index_view() -> CachedFile<Template> {
    let context = HashMap::<String, String>::new();
    CachedFile(Template::render("index", &context))
}

#[get("/static/<file..>", rank=10)]
fn files(file: PathBuf) -> Option<CachedFile<NamedFile>> {
    NamedFile::open(Path::new("static/").join(file)).ok().map(|nf| CachedFile(nf))
}

fn main() {
    rocket::ignite()
        .attach(Template::fairing())
        // .mount("/static", StaticFiles::from("./static"))
        .mount(
            "/",
            routes![index_view, autocomplete_view, empty_search_view, search_view, project_view, project_logo_view, project_logo_svg_view, robots_txt_view, files],
        )
        .launch();
}
