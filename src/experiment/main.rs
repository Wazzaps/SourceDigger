mod ctags_to_diff;
mod repo_to_ctags;
use git2::Repository;
use regex::Regex;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::os::unix::ffi::OsStrExt;

fn osstr_to_regex(s: OsString) -> Regex {
    Regex::new(
        &String::from_utf8(s.as_bytes().to_vec()).expect("Regex contained invalid UTF-8")
    ).expect("Invalid regex")
}

fn read_args() -> (OsString, PathBuf, Regex, Regex) {
    let mut args = std::env::args_os();
    if args.len() != 5 {
        println!("Usage: ./sourcedigger-process <ProjectName> <BareGitRepoPath> <TagRegex> <FileRegex>");
        std::process::exit(1);
    }
    let (_, project_name, repo_path, tag_pattern, file_pattern) = (
        args.next().unwrap(),
        args.next().unwrap(),
        PathBuf::from(args.next().unwrap()),
        osstr_to_regex(args.next().unwrap()),
        osstr_to_regex(args.next().unwrap()),
    );

    (project_name, repo_path, tag_pattern, file_pattern)
}

fn main() {
    // Params
    let (project_name, repo_path, tag_pattern, file_pattern) = read_args();
    let tag_time_sort = true;

    // Open repo
    let db_path = Path::new("sourcedigger-db").join(&project_name);
    let repo = Repository::open(&repo_path).unwrap();

    // Symbols for each object
    let start = Instant::now();
    let _tag_count = repo_to_ctags::repo_to_ctags(
        &project_name,
        &db_path,
        &repo,
        Some(&tag_pattern),
        Some(&file_pattern),
        tag_time_sort
    );
    ctags_to_diff::ctags_to_diff(&repo,
                                 &db_path,Some(&tag_pattern), Some(&file_pattern),
                                 tag_time_sort);
    println!(
        "[footer] Finished update in {}ms",
        start.elapsed().as_millis()
    );
}
