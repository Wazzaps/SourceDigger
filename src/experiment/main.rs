mod ctags_to_diff;
mod repo_to_ctags;
use git2::Repository;
use regex::Regex;
use std::ffi::OsString;
use std::path::Path;
use std::time::Instant;

fn main() {
    // Params
    let project_name = OsString::from("linux");
    let repo_path = Path::new("sources").join("linux.git");
    // let tag_pattern = Regex::new(r"(v\d+)\.?\d\d*\.?\d*").unwrap();
    let tag_pattern = Regex::new(r"(v\d+\.?\d)\d*\.?\d*").unwrap();
    // let tag_pattern = Regex::new(r"(v\d+\.?\d\d*\.?\d*)").unwrap();
    let file_pattern = Regex::new(r"^.*\.[ch]$").unwrap();

    // Open repo
    let db_path = Path::new("sourcedigger-db").join(&project_name);
    let repo = Repository::open(&repo_path).unwrap();

    // Tags for each object
    let start = Instant::now();
    let tag_count = repo_to_ctags::repo_to_ctags(
        &project_name,
        &db_path,
        &repo,
        Some(&tag_pattern),
        Some(&file_pattern),
    );
    ctags_to_diff::ctags_to_diff(&repo,
                                 &db_path,Some(&tag_pattern), Some(&file_pattern));
    println!(
        "[footer] Finished update in {}ms",
        start.elapsed().as_millis()
    );
}
