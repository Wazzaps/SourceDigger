use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use std::os::unix::ffi::OsStrExt;

use git2::{Commit, ObjectType, Oid, Repository, TreeWalkMode, TreeWalkResult};
use hex;
use regex::Regex;
use subprocess;
use subprocess::ExitStatus;
use ctags::{Ctags, TagType};
use std::fs::File;

fn collect_tags(repo: &Repository, pattern: Option<Regex>) -> Vec<String> {
    let tags = repo.tag_names(None).unwrap();
    let tags: Vec<&str> = tags.iter().map(|tag| tag.unwrap()).collect();

    if let Some(pattern) = pattern {
        let mut match_tags = HashSet::new();
        let mut filtered_tags = Vec::new();
        for tag in tags {
            let cap = pattern.captures(tag);
            if let Some(cap) = cap {
                let cap = cap.get(1);
                if let Some(cap) = cap {
                    if match_tags.insert(cap.as_str()) {
                        filtered_tags.push(tag.to_string());
                    }
                }
            }
        }

        filtered_tags
    } else {
        tags.iter().map(|s| s.to_string()).collect()
    }
}

fn tag_to_commit<'repo>(repo: &'repo Repository, tag_name: &str) -> Result<Commit<'repo>, git2::Error> {
    let full_tag = format!("refs/tags/{}", tag_name);
    repo.find_reference(&full_tag)?.peel_to_commit()
}

fn get_objects_in_tags(repo: &Repository, tags: &Vec<String>, file_pattern: Option<Regex>) -> HashSet<Oid> {
    let start = Instant::now();
    println!("[progress_title] Collecting objects");

    let mut objects = HashSet::new();
    let mut file_counter = 0;

    for (i, tag) in tags.iter().enumerate() {
        println!("[progress:{:.2}%] Scanning tag: {}", (i as f64) / (tags.len() as f64) * 100., tag);
        if let Ok(commit) = tag_to_commit(repo, tag) {
            commit.tree().unwrap().walk(TreeWalkMode::PreOrder, |_dir, item| {
                if let Some(file_pattern) = &file_pattern {
                    if file_pattern.is_match(item.name().unwrap()) {
                        objects.insert(item.id());
                    }
                } else {
                    objects.insert(item.id());
                }
                file_counter += 1;
                TreeWalkResult::Ok
            }).unwrap();
        } else {
            println!("[!] Faulty tag: {}", tag);
        }
    }

    println!("[progress:100%] Collected {} objects in {} ms, {} ignored",
             objects.len(), start.elapsed().as_millis(), file_counter - objects.len());

    objects
}

fn write_objects(project_name: &OsStr, repo: &Repository, objects: &HashSet<Oid>) {
    let start = Instant::now();
    println!("[progress_title] Writing objects");

    let objects_len = objects.len();
    let counter = AtomicUsize::new(0);
    let last_print_counter = AtomicUsize::new(0);

    let objects_path = Path::new("signup-db").join(project_name).join("objects");
    std::fs::create_dir_all(&objects_path).unwrap();

    objects.iter().for_each(|obj| {
        let obj_path = &objects_path.join(hex::encode(obj.as_bytes()));
        // TODO: racy code
        if !obj_path.exists() {
            let i = counter.fetch_add(1, Ordering::SeqCst);
            // Print only about once every 0.1% of progress
            if (((i - last_print_counter.load(Ordering::SeqCst)) as f64) / (objects_len as f64)) > 0.001 {
                // I know this is not correct code, but it's for print throttling so i'm fine with this
                last_print_counter.store(i, Ordering::SeqCst);
                println!("[progress:{:.2}%] Writing object: {}", (i as f64) / (objects_len as f64) * 100., hex::encode(&obj.as_bytes()));
            }

            let mut file = std::fs::File::create(obj_path).unwrap();
            let obj = repo.find_object(*obj, Some(ObjectType::Blob)).unwrap();
            let blob = obj.into_blob().unwrap();
            file.write_all(blob.content()).unwrap();
        }
    });

    println!("[progress:100%] Wrote {} objects in {} ms", counter.into_inner(), start.elapsed().as_millis());
}

fn parse_objects(project_name: &OsStr, objects: &HashSet<Oid>) -> PathBuf {
    let start = Instant::now();
    println!("[progress_title] Processing objects");
    let output_file = Path::new("signup-db").join(project_name).join("ctags");

    println!("[progress_estimate] {}ms", objects.len());
    // Flags
    let args: Vec<OsString> = vec![
        "--tag-relative".into(),
        "--language-force=c".into(),
        "--sort=no".into(),

        // Output file
        "-f".into(),
        "ctags".into(),

        // Input files
        "-L".into(),
        "-".into()
    ];

    let mut ctags_proc = subprocess::Exec::cmd("ctags")
        .args(&args)
        .stdin(subprocess::Redirection::Pipe)
        .cwd(
            Path::new("signup-db")
                .join(project_name)
        )
        .popen().unwrap();
    let mut file_stream = BufWriter::new(ctags_proc.stdin.take().unwrap());

    // Objects
    objects
        .iter()
        .map(|o|
            Path::new("objects")
                .join(hex::encode(o.as_bytes()))
                .into_os_string()
        )
        .for_each(|path| {
            file_stream.write_all(path.as_bytes()).unwrap();
            file_stream.write_all("\n".as_bytes()).unwrap();
        });
    drop(file_stream);

    let ctags_status = ctags_proc.wait().unwrap();
    assert_eq!(ctags_status, ExitStatus::Exited(0));

    println!("[progress:100%] Processed {} objects in {} ms", objects.len(), start.elapsed().as_millis());

    output_file
}

fn main() {
    let project_name = OsString::from("vim");
    let repo_name = OsString::from("vim.git");
    let tag_pattern = Regex::new(r"(v\d+)\.?\d\d*\.?\d*").unwrap();
    // let tag_pattern = Regex::new(r"(v\d+\.?\d)\d*\.?\d*").unwrap();
    // let tag_pattern = Regex::new(r"(v\d+\.?\d\d*\.?\d*)").unwrap();
    let file_pattern = Regex::new(r"^.*\.[ch]$").unwrap();

    let repo = Repository::open(Path::new("sources").join(repo_name)).unwrap();

    // Generate ctags file for all file versions in repo
    let tags = collect_tags(&repo, Some(tag_pattern));
    let objects = get_objects_in_tags(&repo, &tags, Some(file_pattern));
    write_objects(&project_name, &repo, &objects);
    let ctags_file = parse_objects(&project_name, &objects);

    // Load resulting ctags file
    let tags = Ctags::new(
        BufReader::new(File::open(ctags_file).unwrap()),
        Some(Path::new("signup-db").join(project_name))
    );

    // Display
    for t in tags.filter(|t| t.tag_type == TagType::Function).take(10) {
        println!("{} @ {}:{}", t.name, t.file, t.line_num.unwrap_or(0));
    }
}