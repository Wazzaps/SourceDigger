use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io::{BufReader, BufWriter, Write, Read};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use ctags::{Ctags, SymbolType};
use git2::{Commit, ObjectType, Oid, Repository, TreeWalkMode, TreeWalkResult};
use regex::Regex;
use std::fs::File;
use subprocess::ExitStatus;

pub fn collect_tags(repo: &Repository, pattern: Option<&Regex>, time_sort: bool) -> Vec<String> {
    let mut tags = vec![];
    let mut timed_tags = vec![];
    repo.tag_foreach(|oid, name| {
        let obj = repo.find_object(oid, None).unwrap();
        let commit = if let Some(tag) = obj.as_tag() {
            tag.target().unwrap().into_commit()
        } else {
            obj.into_commit()
        };

        if let Ok(commit) = commit {
            if time_sort {
                timed_tags.push((
                    commit.time().seconds(),
                    String::from_utf8_lossy(name).split('/').last().unwrap().to_string()
                ));
            } else {
                tags.push(String::from_utf8_lossy(name).split('/').last().unwrap().to_string());
            }
        }
        true
    }).unwrap();

    if time_sort {
        timed_tags.sort_unstable_by_key(|t| t.0);
    } else {
        alphanumeric_sort::sort_path_slice(&mut tags);
    }

    let tags = if time_sort {
        timed_tags.into_iter().map(|t| t.1).collect()
    } else {
        tags
    };

    if let Some(pattern) = pattern {
        let mut match_tags = HashSet::new();
        let mut filtered_tags = Vec::new();
        for tag in tags {
            let cap = pattern.captures(&tag);
            if let Some(cap) = cap {
                let cap = cap.get(1);
                if let Some(cap) = cap {
                    if match_tags.insert(cap.as_str().to_string()) {
                        filtered_tags.push(tag.to_string());
                    }
                }
            }
        }

        filtered_tags
    } else {
        tags
    }
}

fn tag_to_commit<'repo>(
    repo: &'repo Repository,
    tag_name: &str,
) -> Result<Commit<'repo>, git2::Error> {
    let full_tag = format!("refs/tags/{}", tag_name);
    repo.find_reference(&full_tag)?.peel_to_commit()
}

pub fn iter_objects_in_tags<TagCB1, TagCB2, FileCB>(
    repo: &Repository,
    tags: &Vec<&str>,
    file_pattern: Option<&Regex>,
    mut pre_tag_callback: TagCB1,
    mut post_tag_callback: TagCB2,
    mut file_callback: FileCB,
) -> usize
where
    TagCB1: FnMut(usize, &str),
    TagCB2: FnMut(usize, &str),
    FileCB: FnMut(&str, Oid, &str),
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

fn fix_whitespace(src: &str) -> String {
    let pattern1 = Regex::new(r"[ \t]+").unwrap();
    let pattern2 = Regex::new(r"\([ \t]").unwrap();
    let pattern3 = Regex::new(r"[ \t]\)").unwrap();
    let src = pattern1.replace_all(src, " ");
    let src = pattern2.replace_all(src.as_ref(), "(");
    pattern3.replace_all(src.as_ref(), ")").to_string()
}

fn get_func_args_at_line(source_code: &str, line_num: Option<u64>) -> String {
    if let Some(line_num) = line_num {
        let mut result = String::new();
        let mut depth = 0;
        let mut done = false;
        for line in source_code.lines().skip((line_num - 1) as usize).take(20) {
            if done {
                break;
            }
            for c in line.chars() {
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        result.push(c);
                        done = true;
                        break;
                    }
                }
                if depth != 0 {
                    result.push(c);
                }
            }
        }
        if !done {
            result += ", ???)";
        }

        fix_whitespace(&result)
    } else {
        "(???)".to_string()
    }
}

struct RetvalState {
    is_const: bool,
    type_name: String,
    ptr_level: usize,
}

impl RetvalState {
    fn finalize(&self) -> String {
        let mut result = String::new();
        if self.is_const {
            result += "const ";
        }
        if self.type_name.len() != 0 {
            result += &self.type_name;
        } else {
            result += "int";
        }
        for _ in 0..self.ptr_level {
            result += "*";
        }
        result
    }

    fn try_regex(&mut self, pattern: Regex, line: &str) -> bool {
        if let Some(p1_res) = pattern.captures(&line) {
            let (is_const, type_prefix, type_name, ptr_level) =
                (
                    p1_res.get(1).map(|m| !m.as_str().is_empty()).unwrap_or(false),
                    p1_res.get(2).unwrap().as_str(),
                    p1_res.get(3).unwrap().as_str(),
                    p1_res.get(4).unwrap().as_str().len()
                );
            self.is_const = is_const;
            if type_prefix.len() == 0 {
                self.type_name = type_name.to_string();
            } else {
                self.type_name = format!("{} {}", type_prefix, type_name);
            }
            self.ptr_level = ptr_level;
            true
        } else {
            false
        }
    }

    pub fn feed(&mut self, line: &str) -> Option<String> {
        // If we stumble on a comment or define, we're done
        let line_trimmed = line.trim();
        if line_trimmed.starts_with("#") || line_trimmed.starts_with("/") || line_trimmed.contains("*/") {
            return Some(self.finalize());
        }

        let line = line.replace("static", "").replace("inline", "");
        let pattern1 = Regex::new("\\b(const )?(struct|enum|union) ([a-zA-Z$_][a-zA-Z0-9$_]*)\\b[ \t]*(\\**)").unwrap();
        let pattern2 = Regex::new("\\b(const )?(signed|unsigned) ([a-zA-Z$_][a-zA-Z0-9$_]*)\\b[ \t]*(\\**)").unwrap();
        let pattern3 = Regex::new("\\b(const )?()([a-zA-Z$_][a-zA-Z0-9$_]*)\\b[ \t]*(\\**)").unwrap();
        let found = self.try_regex(pattern1, &line)
            || self.try_regex(pattern2, &line)
            || self.try_regex(pattern3, &line);
        if found {
            return Some(self.finalize());
        }
        None
    }
}

fn get_func_ret_at_line(symbol_name: &str, source_code: &str, line_num: Option<u64>) -> String {
    if let Some(line_num) = line_num {

        const CONTEXT_LINES: usize = 10;
        let begin_line_num = (line_num as i64 - CONTEXT_LINES as i64).max(0) as usize;
        let line_count = line_num as usize - begin_line_num;
        let lines = source_code.lines().skip(begin_line_num).take(line_count).collect::<Vec<_>>();
        let mut lines = lines.into_iter().rev();

        let first_line = lines.next().unwrap();
        let first_line_pattern = Regex::new(&format!("(.*)[ \t]*{}[ \t]*\\(", symbol_name)).unwrap();
        let caps = if let Some(caps) = first_line_pattern.captures(first_line) {
            caps
        } else {
            return "unknown_t".to_string();
        };
        let initial_chunk = caps.get(1)
            .expect(&format!("line with function ({}) didn't have open parenthesis: {}", symbol_name, first_line))
            .as_str().trim();

        let mut retval = RetvalState { is_const: false, type_name: "".to_string(), ptr_level: 0 };
        if let Some(result) = retval.feed(initial_chunk) {
            return result;
        } else {
            for line in lines {
                if let Some(term_idx) = line.rfind('}') {
                    if let Some(result) = retval.feed(&line[term_idx..]) {
                        return result;
                    }
                    break
                } else {
                    if let Some(result) = retval.feed(line) {
                        return result;
                    }
                }
            }
        }
    }
    "int".to_string()
}

fn get_var_contents_at_line(source_code: &str, line_num: Option<u64>) -> String {
    if let Some(line_num) = line_num {
        let mut result = String::new();
        let mut depth = 0;
        let mut done = false;
        for line in source_code.lines().skip((line_num - 1) as usize).take(20) {
            if done {
                break;
            }
            for c in line.chars() {
                if c == '=' {
                    depth = 1;
                } else if c == ';' {
                    depth -= 1;
                    if depth <= 0 {
                        done = true;
                        break;
                    }
                }
                if depth != 0 {
                    result.push(c);
                }
            }
        }
        if !done {
            result += " ???";
        }
        let result = " ".to_string() + &fix_whitespace(result.trim());

        result
    } else {
        "= ???".to_string()
    }
}

fn get_extra_info_at_line(symbol_type: SymbolType, symbol_name: &str, source_code: &str, line_num: Option<u64>) -> String {
    match symbol_type {
        SymbolType::Function => get_func_ret_at_line(symbol_name, source_code, line_num) + " {name}" + &get_func_args_at_line(source_code, line_num),
        SymbolType::Variable => get_var_contents_at_line(source_code, line_num),
        // SymbolType::Define => {},
        _ => "".to_string(),
    }
}

pub fn repo_to_ctags(
    project_name: &OsStr,
    db_path: &PathBuf,
    repo: &Repository,
    tag_pattern: Option<&Regex>,
    file_pattern: Option<&Regex>,
    tag_time_sort: bool,
) -> usize {
    let tags = collect_tags(&repo, tag_pattern, tag_time_sort);
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
    let objs_basepath = db_path.join("objects");
    std::fs::create_dir_all(&tags_basepath).unwrap();
    let mut current_inp_file = String::new();
    let mut current_out_file = None;
    let mut current_obj = String::new();
    let mut is_file_skipped = false;

    let last_print_counter = AtomicUsize::new(0);
    let file_counter = AtomicUsize::new(0);
    let mut skip_counter = 0usize;
    let mut sym_counter = 0usize;
    for symbol in symbols {
        if symbol.symbol_type == SymbolType::Unknown {
            continue;
        }
        if current_obj != symbol.file {
            current_obj = symbol.file.clone();
            let out_path = tags_basepath.join(Path::new(&current_obj).file_name().unwrap());
            let inp_path = objs_basepath.join(Path::new(&current_obj).file_name().unwrap());

            if out_path.exists() {
                is_file_skipped = true;
                skip_counter += 1;
            } else {
                let mut inp_contents = vec![];
                File::open(inp_path).unwrap().read_to_end(&mut inp_contents).unwrap();

                let _ = std::mem::replace(&mut current_inp_file, String::from_utf8_lossy(&inp_contents).to_string());
                current_out_file.replace(File::create(out_path).unwrap());
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
            current_out_file.as_mut().unwrap().write_all(
                format!(
                    "{}\t{:?}\t{}\t{}\n",
                    symbol.name,
                    symbol.symbol_type,
                    symbol.line_num.unwrap_or(0),
                    get_extra_info_at_line(symbol.symbol_type, &symbol.name, &current_inp_file, symbol.line_num)
                )
                    .as_bytes(),
            ).unwrap();
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
