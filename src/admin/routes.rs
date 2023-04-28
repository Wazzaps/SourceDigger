use rocket_contrib::templates::Template;
use std::collections::HashMap;
use rocket::request::Form;
use crate::data::{Project, ProjectRepo, ProjectIndex, ProjectStats};

#[get("/")]
pub fn admin_index() -> Template {
    let context = HashMap::<String, String>::new();
    Template::render("admin/index", &context)
}

#[get("/add_repo")]
pub fn admin_add_repo() -> Template {
    let context = HashMap::<String, String>::new();
    Template::render("admin/add_repo", &context)
}

#[derive(FromForm, Debug)]
pub struct AddRepo {
    repo_url: String,
}

#[post("/add_repo", data = "<form_data>")]
pub fn admin_add_repo_post(form_data: Form<AddRepo>) -> Template {
    if !form_data.repo_url.ends_with(".git") {
        let mut context = HashMap::<String, String>::new();
        context.insert("error".to_string(), "Invalid URL".to_string());
        Template::render("admin/add_repo", &context)
    } else {
        let context = HashMap::<String, String>::new();
        println!("{:?}", form_data);
        Template::render("admin/add_repo_clone_progress", &context)
    }
}

#[get("/repo/<repo_name>")]
pub fn admin_view_repo(repo_name: String) -> Template {
    // let mut context = HashMap::<String, String>::new();
    let proj = Project {
        repo: ProjectRepo { name: "git".to_string(), origin: "git@github.com:git/git.git".to_string() },
        index: ProjectIndex { initial_ver: "v0.99".to_string(), latest_ver: "v2.9.5".to_string() },
        stats: ProjectStats { searches: 1111, autocompletes: 2222 }
    };
    // context.insert("repo_name".to_string(), repo_name);
    Template::render("admin/view_repo", &proj)
}
