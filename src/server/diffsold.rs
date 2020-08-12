// use rayon::prelude::*;
use regex::Regex;
use std::fs::{read_dir, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::atomic::{Ordering, AtomicU64};
use std::time::Instant;
use std::collections::HashMap;

lazy_static!{
    static ref DIFFS_LIST: HashMap<String, HashMap<String, Vec<u8>>> = {
        let mut map = HashMap::new();
        for project in std::fs::read_dir("sourcedigger-db").unwrap() {
            let project = project.unwrap();
            // let project = project.unwrap().file_name().into_string().unwrap();
            let mut db = HashMap::new();
            if let Ok(read_dir) = std::fs::read_dir(project.path().join("diffs")) {
                for tag in read_dir {
                    let tag = tag.unwrap();
                    if let Ok(diff_contents) = std::fs::read(tag.path()) {
                        db.insert(tag.file_name().into_string().unwrap(), diff_contents);
                    }
                }
                map.insert(
                    project.file_name().into_string().unwrap(),
                    db
                );
            }
        }
        map
    };
}

pub fn get_diffs(project: String, query: String, count: u64) -> Receiver<String> {
    assert!(!project.contains("/"));
    let (send, recv) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let start = Instant::now();
        // Gather git tags
        let tags = read_dir(Path::new("sourcedigger-db").join(&project).join("diffs")).unwrap();
        let mut tags = tags
            .map(|t| t.unwrap().file_name().to_string_lossy().to_string())
            .collect::<Vec<String>>();
        alphanumeric_sort::sort_path_slice(&mut tags);

        let counter = AtomicU64::new(0);
        if let Ok(regex) = Regex::new(&query) {
            tags.iter()
                // .par_bridge()
                // .for_each_with(send, |send, tag| {
                .for_each(|tag| {
                    let start = Instant::now();
                    let mut did_show_version = false;
                    // let db = BufReader::new(
                    //     File::open(
                    //         Path::new("sourcedigger-db")
                    //             .join(&project)
                    //             .join("diffs")
                    //             .join(&tag),
                    //     )
                    //     .unwrap(),
                    // );
                    let db = &DIFFS_LIST[&project][tag];
                    for line in db.split(|c| *c == '\n' as u8) {
                        if counter.load(Ordering::SeqCst) >= count || line.len() == 0 {
                            break;
                        }

                        // let line = line.unwrap();
                        let line = String::from_utf8_lossy(&line).to_string();
                        let mut components = line.split('\t');
                        let (action, name, sym_type, file, line) = (
                            components.next().unwrap(),
                            components.next().unwrap(),
                            components.next().unwrap(),
                            components.next().unwrap(),
                            components.next().unwrap(),
                        );

                        if regex.is_match(name) {
                            if !did_show_version {
                                did_show_version = true;
                                send.send(format!("<h2 class=h>{}</h2>\n", &tag)).unwrap();
                            }

                            send.send(format!(
                                // <span class={6}>{6}</span>
                                "<div class={0}><span class={6}>{6}</span><a href=\"diffs?q={1}\">{2}<span>{1}</span>{3}</a><hr><a>{4}:{5}</a></div>\n",
                                match action.as_ref() {
                                    "Add" => "a",
                                    "Remove" => "r",
                                    "Change" => "c",
                                    "Move" => "m",
                                    _ => "u"
                                },
                                &name,
                                match sym_type.as_ref() {
                                    "Function" => "unk_t ",
                                    "Define" => "#define ",
                                    "Variable" => "unk_t ",
                                    _ => ""
                                },
                                match sym_type.as_ref() {
                                    "Function" => "()",
                                    "Define" => "",
                                    "Variable" => " = ?",
                                    _ => ""
                                },
                                &file,
                                &line,
                                match sym_type.as_ref() {
                                    "Function" => "f",
                                    "Define" => "d",
                                    "Variable" => "v",
                                    _ => sym_type.as_ref()
                                },
                            )).unwrap();
                            counter.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    println!("- {} took {:?}", &tag, start.elapsed());
                });
        }
        println!("- took {:?}", start.elapsed());
    });

    recv
}
