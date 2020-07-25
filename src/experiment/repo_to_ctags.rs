use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io::{BufReader, BufWriter, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use ctags::Ctags;
use git2::{Commit, ObjectType, Oid, Repository, TreeWalkMode, TreeWalkResult};
use regex::Regex;
use std::fs::File;
use subprocess::ExitStatus;

pub fn collect_tags(repo: &Repository, pattern: Option<&Regex>) -> Vec<String> {
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

fn tag_to_commit<'repo>(
    repo: &'repo Repository,
    tag_name: &str,
) -> Result<Commit<'repo>, git2::Error> {
    let full_tag = format!("refs/tags/{}", tag_name);
    repo.find_reference(&full_tag)?.peel_to_commit()
}

pub fn iter_objects_in_tags<TAGCB1, TAGCB2, FILECB>(
    repo: &Repository,
    tags: &Vec<&str>,
    file_pattern: Option<&Regex>,
    mut pre_tag_callback: TAGCB1,
    mut post_tag_callback: TAGCB2,
    mut file_callback: FILECB,
) -> usize
where
    TAGCB1: FnMut(usize, &str),
    TAGCB2: FnMut(usize, &str),
    FILECB: FnMut(&str, Oid, &str),
{
    let mut obj_count = 0usize;

    for (i, tag) in tags.iter().enumerate() {
        if let Ok(commit) = tag_to_commit(repo, tag) {
            pre_tag_callback(i, tag);
            commit
                .tree()
                .unwrap()
                .walk(TreeWalkMode::PreOrder, |dir, item| {
                    if let Some(file_pattern) = &file_pattern {
                        if file_pattern.is_match(item.name().unwrap()) {
                            file_callback(tag, item.id(), &format!("{}{}", dir, item.name().unwrap()));
                        }
                    } else {
                        file_callback(tag, item.id(), item.name().unwrap());
                    }
                    obj_count += 1;
                    TreeWalkResult::Ok
                })
                .unwrap();
            post_tag_callback(i, tag);
        } else {
            println!("[!] Faulty tag: {}", tag);
        }
    }

    obj_count
}

fn collect_objects(
    repo: &Repository,
    tags: &Vec<&str>,
    file_pattern: Option<&Regex>,
) -> HashSet<Oid> {
    let start = Instant::now();
    println!("[progress_title] Collecting objects");

    let mut objects = HashSet::new();
    let obj_count = iter_objects_in_tags(repo, tags, file_pattern, |i, tag_name| {
        println!(
            "[progress:{:.2}%] Scanning tag: {}",
            (i as f64) / (tags.len() as f64) * 100.,
            tag_name
        );
    }, |_, _| {
    }, |_tag_name, obj_id, _file_path| {
        objects.insert(obj_id);
    });

    println!(
        "[progress:100%] Collected {} objects in {} ms, {} ignored",
        objects.len(),
        start.elapsed().as_millis(),
        obj_count - objects.len()
    );

    objects
}

fn write_objects(project_name: &OsStr, repo: &Repository, objects: &HashSet<Oid>) -> HashSet<Oid> {
    let start = Instant::now();
    println!("[progress_title] Extracting objects");

    let mut created_objects = HashSet::<Oid>::new();
    let objects_len = objects.len();
    let counter = AtomicUsize::new(0);
    let last_print_counter = AtomicUsize::new(0);

    let objects_path = Path::new("sourcedigger-db").join(project_name).join("objects");
    std::fs::create_dir_all(&objects_path).unwrap();

    objects.iter().for_each(|obj| {
        let obj_path = &objects_path.join(hex::encode(obj.as_bytes()));
        // TODO: racy code
        if !obj_path.exists() {
            created_objects.insert(obj.clone());
            // Print only about once every 0.1% of progress
            let i = counter.fetch_add(1, Ordering::SeqCst);
            if (((i - last_print_counter.load(Ordering::SeqCst)) as f64) / (objects_len as f64))
                > 0.001
            {
                // I know this is not correct code, but it's for print throttling so i'm fine with this
                last_print_counter.store(i, Ordering::SeqCst);
                println!(
                    "[progress:{:.2}%] Extracting object: {}",
                    (i as f64) / (objects_len as f64) * 100.,
                    hex::encode(&obj.as_bytes())
                );
            }

            let mut file = std::fs::File::create(obj_path).unwrap();
            let obj = repo.find_object(*obj, Some(ObjectType::Blob)).unwrap();
            let blob = obj.into_blob().unwrap();
            file.write_all(blob.content()).unwrap();
        }
    });

    println!(
        "[progress:100%] Extracted {} objects in {} ms",
        counter.into_inner(),
        start.elapsed().as_millis()
    );

    created_objects
}

fn parse_objects(project_name: &OsStr, objects: &HashSet<Oid>) -> PathBuf {
    let start = Instant::now();
    println!("[progress_title] Processing objects");
    let output_file = Path::new("sourcedigger-db").join(project_name).join("ctags");

    println!("[progress_estimate] {}ms", objects.len().max(1));
    // Flags
    let args: Vec<OsString> = vec![
        "--tag-relative".into(),
        "--language-force=c".into(),
        "--sort=no".into(),
        "--append=yes".into(),
        // Output file
        "-f".into(),
        "ctags".into(),
        // Input files
        "-L".into(),
        "-".into(),
    ];

    let mut ctags_proc = subprocess::Exec::cmd("ctags")
        .args(&args)
        .stdin(subprocess::Redirection::Pipe)
        .cwd(Path::new("sourcedigger-db").join(project_name))
        .popen()
        .unwrap();
    let mut file_stream = BufWriter::new(ctags_proc.stdin.take().unwrap());

    // Objects
    objects
        .iter()
        .map(|o| {
            Path::new("objects")
                .join(hex::encode(o.as_bytes()))
                .into_os_string()
        })
        .for_each(|path| {
            file_stream.write_all(path.as_bytes()).unwrap();
            file_stream.write_all("\n".as_bytes()).unwrap();
        });
    drop(file_stream);

    let ctags_status = ctags_proc.wait().unwrap();
    assert_eq!(ctags_status, ExitStatus::Exited(0));

    println!(
        "[progress:100%] Processed {} objects in {} ms",
        objects.len(),
        start.elapsed().as_millis()
    );

    output_file
}

pub fn repo_to_ctags(
    project_name: &OsStr,
    db_path: &PathBuf,
    repo: &Repository,
    tag_pattern: Option<&Regex>,
    file_pattern: Option<&Regex>,
) -> usize {
    let tags = collect_tags(&repo, tag_pattern);
    let objects = collect_objects(&repo, &tags.iter().map(String::as_str).collect::<Vec<_>>(), file_pattern);
    let new_objects = write_objects(project_name, &repo, &objects);
    let ctags_file = parse_objects(project_name, &new_objects);

    // Split ctags per git object
    let start = Instant::now();
    println!("[progress_title] Organizing symbols by object");
    let symbols = Ctags::new(
        BufReader::new(File::open(&ctags_file).unwrap()),
        Some(&db_path),
    );

    let tags_basepath = db_path.join("tags");
    std::fs::create_dir_all(&tags_basepath).unwrap();
    let mut current_file = None;
    let mut current_obj = String::new();
    let mut is_file_skipped = false;

    let last_print_counter = AtomicUsize::new(0);
    let file_counter = AtomicUsize::new(0);
    let mut skip_counter = 0usize;
    let mut sym_counter = 0usize;
    for symbol in symbols {
        if current_obj != symbol.file {
            current_obj = symbol.file.clone();
            let out_path = tags_basepath.join(Path::new(&current_obj).file_name().unwrap());

            if out_path.exists() {
                is_file_skipped = true;
                skip_counter += 1;
            } else {
                current_file.replace(File::create(out_path).unwrap());
                is_file_skipped = false;

                // Print only about once every 0.1% of progress
                let i = file_counter.fetch_add(1, Ordering::SeqCst);
                if (((i - last_print_counter.load(Ordering::SeqCst)) as f64)
                    / (new_objects.len() as f64))
                    > 0.001
                {
                    // I know this is not correct code, but it's for print throttling so i'm fine with this
                    last_print_counter.store(i, Ordering::SeqCst);
                    println!(
                        "[progress:{:.2}%] Organizing object's symbols: {}",
                        (i as f64) / (new_objects.len() as f64) * 100.,
                        current_obj
                    );
                }
            }
        }
        if !is_file_skipped {
            &current_file.as_mut().unwrap().write_all(
                format!(
                    "{}\t{:?}\t{}\n",
                    symbol.name,
                    symbol.symbol_type,
                    symbol.line_num.unwrap_or(0),
                )
                .as_bytes(),
            );
            sym_counter += 1;
        }
    }
    println!(
        "[progress:100%] Organized {} symbols of {} objects ({} objects skipped) in {} ms",
        sym_counter,
        file_counter.into_inner(),
        skip_counter,
        start.elapsed().as_millis()
    );

    sym_counter
}
