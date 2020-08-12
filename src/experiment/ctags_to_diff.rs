use crate::repo_to_ctags;
use ctags::SymbolType;
use git2::{Oid, Repository};
use regex::Regex;
use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet, VecDeque, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TagAction {
    Add,
    Remove,
    Modify,
}

/**
 * Unique identifier of symbol across versions
 */
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct TagID {
    name: String,
    tag_type: SymbolType,
}

impl PartialOrd for TagID {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(alphanumeric_sort::compare_str(&self.name.to_ascii_lowercase(), &other.name.to_ascii_lowercase()))
    }
}

impl Ord for TagID {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        alphanumeric_sort::compare_str(&self.name.to_ascii_lowercase(), &other.name.to_ascii_lowercase())
    }
}

/**
 * Information that may change across versions
 */
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TagData {
    file: String,
    line_num: u64,

    /// Changing this will trigger a "modified" event
    extra_data: String,
}

type TagHashMap = HashMap<TagID, Vec<TagData>>;

fn load_tags<F>(db_path: &PathBuf, obj_id: Oid, mut callback: F)
where
    F: FnMut(&str, u64, SymbolType, Option<&str>),
{
    let tags;
    if let Ok(file) = File::open(
        Path::new(db_path)
            .join("tags")
            .join(hex::encode(obj_id.as_bytes())),
    ) {
        tags = BufReader::new(file).lines();
    } else {
        return;
    }

    for line in tags {
        let line = line.unwrap();
        let mut parts = line.splitn(4, "\t");
        let (name, tag_type, line_num, extra_data) = (
            parts.next().unwrap(),
            match parts.next().unwrap() {
                "Function" => SymbolType::Function,
                "Define" => SymbolType::Define,
                "Variable" => SymbolType::Variable,
                _ => SymbolType::Unknown,
            },
            parts.next().unwrap().parse::<u64>().unwrap(),
            parts.next(),
        );
        callback(name, line_num, tag_type, extra_data);
        // println!("{} {} {:?} {:?}", name, line_num, tag_type, extra_data);
    }
}

pub fn ctags_to_diff(
    repo: &Repository,
    db_path: &PathBuf,
    tag_pattern: Option<&Regex>,
    file_pattern: Option<&Regex>,
    tag_time_sort: bool,
) {
    let start = Instant::now();
    println!("[progress_title] Creating comparison files");

    let ctags_maps: Mutex<VecDeque<TagHashMap>> =
        Mutex::new(vec![HashMap::new(), HashMap::new()].into());
    let mut current_tag: String = "initial".to_string();
    let file_counter = AtomicUsize::new(0);
    let diff_counter = AtomicUsize::new(0);

    let mut create_diff = |ctags_maps: &mut VecDeque<TagHashMap>, next_tag_name: &str| {
        std::fs::create_dir_all(Path::new(db_path).join("diffs")).unwrap();
        let diff_path = Path::new(db_path)
            .join("diffs")
            .join(next_tag_name.replace("/", "-"));
        if !diff_path.exists() {
            let mut diffs: Vec<(TagAction, TagID, TagData)> = vec![];
            let prev_ctags = ctags_maps.back().unwrap();
            let new_ctags = ctags_maps.front().unwrap();

            let prev_ids: BTreeSet<TagID> = prev_ctags.keys().cloned().collect();
            let new_ids: BTreeSet<TagID> = new_ctags.keys().cloned().collect();

            let added_ids = new_ids.difference(&prev_ids);
            let common_ids = prev_ids.intersection(&new_ids);
            let removed_ids = prev_ids.difference(&new_ids);

            let common_count = common_ids.count();
            if common_count != 0 {
                println!("TODO: ignoring {} changes", common_count);
            }

            for removed_id in removed_ids {
                for removed_data in prev_ctags[removed_id].iter() {
                    diff_counter.fetch_add(1, Ordering::SeqCst);
                    diffs.push((TagAction::Remove, removed_id.clone(), removed_data.clone()));
                }
            }
            for added_id in added_ids {
                for added_data in new_ctags[added_id].iter() {
                    diff_counter.fetch_add(1, Ordering::SeqCst);
                    diffs.push((TagAction::Add, added_id.clone(), added_data.clone()));
                }
            }

            let mut out_file = BufWriter::new(File::create(diff_path).unwrap());
            for (diff_act, diff_id, diff_data) in diffs.iter() {
                out_file.write_all(format!(
                    "{}\t{}\t{:?}\t{}\t{}\t{}\n",
                    match diff_act {
                        TagAction::Add => "a",
                        TagAction::Remove => "r",
                        TagAction::Modify => "m",
                    },
                    diff_id.name,
                    diff_id.tag_type,
                    diff_data.file,
                    diff_data.line_num,
                    diff_data.extra_data
                ).as_bytes()).unwrap();
            }
        }

        // Shift vecs
        ctags_maps.pop_back().unwrap();
        ctags_maps.push_front(HashMap::new());
        current_tag = next_tag_name.to_string();
    };

    let tags = repo_to_ctags::collect_tags(&repo, tag_pattern, tag_time_sort);
    let non_existent_tags = tags.iter().enumerate().filter_map(|(i, tag)| {
        let diff_path = Path::new(db_path)
            .join("diffs")
            .join(tag.replace("/", "-"));
        if diff_path.exists() {
            None
        } else {
            Some(i)
        }
    }).collect::<HashSet<_>>();
    let tags_to_compute = tags.iter().enumerate().filter_map(|(i, tag)| {
        if non_existent_tags.contains(&i) || non_existent_tags.contains(&(i + 1)) {
            Some(tag.as_str())
        } else {
            None
        }
    }).collect::<Vec<_>>();
    let _obj_count = repo_to_ctags::iter_objects_in_tags(
        repo,
        &tags_to_compute,
        file_pattern,
        |i, tag_name| {
            println!(
                "[progress:{:.2}%] Comparing tag: {}",
                (i as f64) / (tags_to_compute.len() as f64) * 100.,
                tag_name
            );
        },
        |i, tag_name| {
            let mut ctags_maps = ctags_maps.lock().unwrap();
            println!(
                "[progress:{:.2}%] Saving comparison for tag: {}",
                ((i * 2 + 1) as f64) / ((tags_to_compute.len() * 2) as f64) * 100.,
                tag_name
            );
            create_diff(ctags_maps.borrow_mut(), tag_name);
        },
        |_tag_name, obj_id, file_path| {
            let mut ctags_maps = ctags_maps.lock().unwrap();
            file_counter.fetch_add(1, Ordering::SeqCst);

            load_tags(db_path, obj_id, |name, line_num, tag_type, extra_data| {
                ctags_maps
                    .front_mut()
                    .unwrap()
                    .entry(TagID {
                        name: name.to_string(),
                        tag_type,
                    })
                    .or_insert_with(Vec::new)
                    .push(TagData {
                        file: file_path.to_string(),
                        line_num,
                        extra_data: extra_data.unwrap_or("").to_string(),
                    });
            });
        },
    );

    println!(
        "[progress:100%] Created {} comparisons of {} objects in {} ms",
        diff_counter.into_inner(),
        file_counter.into_inner(),
        start.elapsed().as_millis()
    );
}
