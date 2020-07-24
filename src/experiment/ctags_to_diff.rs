use git2::Repository;
use std::time::Instant;
use regex::Regex;
use crate::repo_to_ctags;
use ctags::TagType;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::borrow::BorrowMut;
use std::sync::atomic::{AtomicUsize, Ordering};

/**
 * Unique identifier of symbol across versions
 */
#[derive(Debug, Eq, PartialEq, Hash)]
struct TagID {
    name: String,
    tag_type: TagType,
}

/**
 * Information that may change across versions
 */
#[derive(Debug, Eq, PartialEq)]
struct TagData {
    file: String,
    line_num: u64,

    /// Changing this will trigger a "modified" event
    extra_data: String,
}

type TagHashMap = HashMap<TagID, Vec<TagData>>;

pub fn ctags_to_diff(
    repo: &Repository,
    tag_pattern: Option<&Regex>,
    file_pattern: Option<&Regex>,
) {
    let start = Instant::now();
    println!("[progress_title] Creating comparison files [UNIMPLEMENTED]");

    let mut ctags_maps: Mutex<VecDeque<TagHashMap>> = Mutex::new(vec![HashMap::new(), HashMap::new()].into());
    let mut current_tag: String = "initial".to_string();
    let file_counter = AtomicUsize::new(0);

    let mut create_diff = |ctags_maps: &mut VecDeque<TagHashMap>, next_tag_name: &str| {
        // println!("'{}.diff': '{}' -> '{}' [{}]", next_tag_name, current_tag, next_tag_name, ctags_maps.front().unwrap().len());
        // println!("{:?}", ctags_maps.front().unwrap());
        ctags_maps.pop_back().unwrap();
        ctags_maps.push_front(HashMap::new());
        current_tag = next_tag_name.to_string();
    };

    let tags = repo_to_ctags::collect_tags(&repo, tag_pattern);
    let _obj_count = repo_to_ctags::iter_objects_in_tags(repo, &tags, file_pattern, |i, tag_name| {
        println!(
            "[progress:{:.2}%] Comparing tag: {}",
            (i as f64) / (tags.len() as f64) * 100.,
            tag_name
        );
    }, |i, tag_name| {
        let mut ctags_maps = ctags_maps.lock().unwrap();
        create_diff(ctags_maps.borrow_mut(), tag_name);
    }, |tag_name, obj_id, file_path| {
        let mut ctags_maps = ctags_maps.lock().unwrap();
        // println!("{}: {} [{}]", tag_name, file_path, hex::encode(obj_id.as_bytes()));
        file_counter.fetch_add(1, Ordering::SeqCst);
        ctags_maps.front_mut().unwrap().entry(TagID {
            name: file_path.to_string(),
            tag_type: TagType::Function
        }).or_insert_with(Vec::new).push(TagData {
            file: file_path.to_string(),
            line_num: 0,
            extra_data: String::new(),
        });
    });

    println!(
        "[progress:100%] Created {} comparisons of {} objects in {} ms",
        0,
        file_counter.into_inner(),
        start.elapsed().as_millis()
    );
}