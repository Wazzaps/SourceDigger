use std::path::Path;
use std::io::{BufReader, BufRead};
use std::sync::mpsc::Receiver;
use std::time::Instant;

pub fn complete(project: String, query: String, count: u64) -> Receiver<String> {
    assert!(!project.contains("/"));
    let (send, recv) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let start = Instant::now();

        let rg_query = if query.chars().all(|x| x.is_alphanumeric() || x == '_') {
            format!(r"^{}$", &query)
        } else {
            format!(r"^({})$", &query)
        };

        let rg_args = ["--smart-case", &rg_query, "autocomplete_db"];

        println!("autocomplete rg_args: {:?}", &rg_args);
        let mut rg_proc = subprocess::Exec::cmd("rg")
            .args(&rg_args)
            .stdout(subprocess::Redirection::Pipe)
            .cwd(Path::new("sourcedigger-db").join(&project))
            .popen()
            .unwrap();

        let rg_out = rg_proc.stdout.take().unwrap();
        for line in BufReader::new(rg_out).lines().take(count as usize) {
            send.send(line.unwrap() + "\n").unwrap();
        }
        println!("- autocomplete took {:?}", start.elapsed());
    });

    recv
}

