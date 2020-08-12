use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::RwLock;
use std::time::Instant;
use std::collections::HashMap;
use regex::Regex;
use crate::data::ProjectRepo;
use std::fs::File;

lazy_static! {
    static ref PROJECTS: RwLock<HashMap<String, ProjectRepo>> = RwLock::new(HashMap::new());
}

pub fn get_diffs(project: String, query: String, actions: String, types: String, _count: u64) -> Receiver<String> {
    assert!(!project.contains("/"));
    let (send, recv) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let start = Instant::now();

        send.send("<link rel=\"stylesheet\" href=/static/results-inner.css>".to_string()).unwrap();

        let actions = if actions == "arm" {
            ".".to_string()
        } else {
            format!("[{}]", actions)
        };

        let types_query = types.chars().map(|c| match c {
            'f' => "Function",
            'd' => "Define",
            'v' => "Variable",
            _ => "",
        }).collect::<Vec<_>>().join("|");

        let types_query = if types_query.len() == 0 {
            ".*".to_string()
        } else {
            format!("({})", types_query)
        };

        let query_expander = Regex::new("(^|[^.\\]])([*+])").unwrap();

        let query = query_expander.replace_all(&query, "$1.$2");
        let rg_query = if query.chars().all(|x| x.is_alphanumeric() || x == '_') {
            format!(r"^{}\t{}\t{}", &actions, &query, &types_query)
        } else {
            format!(r"^{}\t({})\t{}", &actions, &query, &types_query)
        };

        // let replace_dots = Regex::new(r#"."#).unwrap();
        let rg_query = rg_query.replace(".", "[^\\t]");

        let rg_args = [
            "10s", // FIXME: hardcoded query timeout
            "rg", "--heading", "--smart-case", &rg_query
        ];

        println!("rg_args: {:?}", &rg_args);
        let mut rg_proc = subprocess::Exec::cmd("timeout")
            .args(&rg_args)
            .stdout(subprocess::Redirection::Pipe)
            .cwd(Path::new("sourcedigger-db").join(&project).join("diffs"))
            .popen()
            .unwrap();

        let mut output = HashMap::new();
        let rg_out = rg_proc.stdout.take().unwrap();
        let rg_lines: Vec<String> = BufReader::new(&rg_out).lines().map(|l| l.unwrap()).collect();

        println!("- rg took {:?}", start.elapsed());

        let project_data: Option<ProjectRepo> = PROJECTS.read().unwrap().get(&project).map(|r| r.clone());
        let project_data = if let Some(data) = project_data {
            data
        } else {
            let mut projects = PROJECTS.write().unwrap();

            let mut data_string = String::new();
            File::open(Path::new("sourcedigger-db").join(&project).join("config.toml"))
                .expect("Failed to open config")
                .read_to_string(&mut data_string)
                .expect("Failed to read config");

            let project_data: ProjectRepo = toml::from_str(&data_string).expect("Invalid config");
            projects.insert(project.clone(), project_data.clone());
            project_data
        };

        for subslice in rg_lines.split(|s| s == "") {
            let (tag_name, lines) = match subslice.split_first() {
                None => continue,
                Some(s) => s,
            };
            let mut tag_output = format!("<h2 class=h>{}</h2>\n", &tag_name);

            for line in lines {
                let mut components = line.split('\t');
                let (action, name, sym_type, file, line, extra) = (
                    components.next().unwrap(),
                    components.next().unwrap(),
                    components.next().unwrap(),
                    components.next().unwrap(),
                    components.next().unwrap(),
                    components.next().unwrap_or(""),
                );

                tag_output += &format!(
                    "<div class={0}><a href=# class={6}>{6}</a><a href=# class={0}>{7}</a><a href=\"diffs?q={1}\">{2}<span>{1}</span>{3}</a><hr><a href=\"{8}\">{4}:{5}</a></div>\n",
                    match action.as_ref() {
                        "a" => "a",
                        "r" => "r",
                        "m" => "m",
                        _ => "u"
                    },
                    &name,
                    match sym_type.as_ref() {
                        "Function" => "unk_t ",
                        "Define" => "#define ",
                        "Variable" => "unk_t ",
                        _ => ""
                    },
                    extra,
                    &file,
                    &line,
                    match sym_type.as_ref() {
                        "Function" => "f",
                        "Define" => "d",
                        "Variable" => "v",
                        _ => sym_type.as_ref()
                    },
                    match action.as_ref() {
                        "a" => "+",
                        "r" => "-",
                        "m" => "~",
                        _ => "u"
                    },
                    project_data.source_viewer
                        .replace("{tag}", &tag_name)
                        .replace("{path}", &file)
                        .replace("{line}", &format!("{}", line))
                )
            }

            output.insert(tag_name.clone(), tag_output);
        }

        let mut sorted_tags: Vec<String> = output.keys().cloned().collect();
        alphanumeric_sort::sort_path_slice_rev(&mut sorted_tags);

        for tag in sorted_tags {
            send.send(output.remove(&tag).unwrap()).unwrap();
        }
        println!("- request took {:?}", start.elapsed());
    });

    recv
}
