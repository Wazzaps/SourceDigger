use std::fs::File;
use std::collections::HashMap;
use std::path::Path;
use std::io::Read;
use regex::bytes::Regex;
use rocket::response::Stream;
use std::sync::mpsc::Receiver;

lazy_static! {
    static ref SYMBOLS_LIST: HashMap<String, Vec<u8>> = {
        let mut map = HashMap::new();
        for project in std::fs::read_dir("sourcedigger-db").unwrap() {
            let project = project.unwrap().file_name().into_string().unwrap();
            if let Ok(mut db_file) = File::open(Path::new("sourcedigger-db").join(&project).join("autocomplete_db")) {
                let mut db = vec![];
                if !db_file.read_to_end(&mut db).is_ok() {
                    continue;
                }
                map.insert(
                    project.clone(),
                    db
                );
            }
        }
        map
    };
}

pub fn init() {
    // Initialize lazy variable
    let _ = SYMBOLS_LIST.len();
}

pub fn complete(project: String, query: String, count: usize) -> Receiver<String> {
    let (send, recv) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let symbols_list: &HashMap<String, Vec<u8>> = &SYMBOLS_LIST;
        let mut counter = 0;
        if let Ok(regex) = Regex::new(&query) {
            if let Some(project_syms) = symbols_list.get(&project) {
                for symbol in project_syms.split(|c| *c == '\n' as u8) {
                    if counter >= count {
                        break;
                    }

                    if regex.is_match(symbol) {
                        send.send(String::from_utf8_lossy(symbol).to_string() + "\n").unwrap();
                        counter += 1;
                    }
                }
            }
        }
    });

    recv
}

