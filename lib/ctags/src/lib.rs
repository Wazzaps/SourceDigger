use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use crate::{Ctags, Tag, TagType};
    use std::fs::File;
    use std::io::Read;

    #[test]
    fn smoke() {
        let mut source = vec![];
        File::open("./test_res/smoke/ctags")
            .unwrap()
            .read_to_end(&mut source)
            .unwrap();

        let mut tags = Ctags::new(source.as_slice(), Some("./test_res/smoke"));
        assert_eq!(
            tags.next(),
            Some(Tag {
                name: "Hello".into(),
                file: "example.c".into(),
                expression: "/^int Hello() {$/".into(),
                line_num: Some(3),
                tag_type: TagType::Function
            })
        );
        assert_eq!(
            tags.next(),
            Some(Tag {
                name: "World".into(),
                file: "example.c".into(),
                expression: "/^int World() {$/".into(),
                line_num: Some(6),
                tag_type: TagType::Function
            })
        );
        assert_eq!(
            tags.next(),
            Some(Tag {
                name: "FOOBAR".into(),
                file: "example.c".into(),
                expression: "1".into(),
                line_num: Some(1),
                tag_type: TagType::Define
            })
        );
    }
}

pub struct Ctags<R: Read> {
    source: BufReader<R>,
    code_dir: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Tag {
    pub name: String,
    pub file: String,
    pub expression: String,
    pub line_num: Option<u64>,
    pub tag_type: TagType,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum TagType {
    Unknown = 0,
    Function = 1,
    Define = 2,
}

impl<R: Read> Ctags<R> {
    pub fn new<P: Into<PathBuf>>(ctags_contents: R, code_dir: Option<P>) -> Self {
        Self {
            source: BufReader::new(ctags_contents),
            code_dir: code_dir.map(|c| c.into()),
        }
    }
}

impl<R: Read> Iterator for Ctags<R> {
    type Item = Tag;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        loop {
            if let Ok(bytes_read) = self.source.read_line(&mut line) {
                if bytes_read == 0 {
                    return None;
                }
                // Ignore lines starting with '!'
                if !line.starts_with("!") {
                    break;
                }
                line.clear();
            }
        }

        let (name, file, expression) = {
            let mut it = line.splitn(3, "\t");
            (it.next()?, it.next()?, it.next()?)
        };

        let (expression, extra) = {
            let mut it = expression.splitn(2, ";\"");
            (it.next()?, it.next()?)
        };

        let mut extra = extra.trim_matches('\t').trim_end_matches("\n").split("\t");

        let tag_type = match extra.next().unwrap_or("") {
            "f" => TagType::Function,
            "d" => TagType::Define,
            _ => TagType::Unknown,
        };

        if expression.starts_with("/^") && expression.ends_with("$/") {
            // regex match
            // TODO: Use regex, not string matching
            if let Some(code_dir) = &self.code_dir {
                if let Ok(mut code_file) = File::open(code_dir.join(file)) {
                    // Read file
                    // TODO: Keep FD open and continue from last position
                    let mut code_contents = vec![];
                    code_file.read_to_end(&mut code_contents).unwrap();
                    let code_contents = String::from_utf8_lossy(&code_contents);

                    let line_num =
                        code_contents
                            .lines()
                            .enumerate()
                            .find_map(|(line, contents)| {
                                if *contents == expression[2..expression.len() - 2] {
                                    Some((line as u64) + 1)
                                } else {
                                    None
                                }
                            });

                    return Some(Tag {
                        name: name.to_string(),
                        file: file.to_string(),
                        expression: expression.to_string(),
                        line_num,
                        tag_type,
                    });
                }
            }
        } else if expression.chars().all(char::is_numeric) {
            // explicit line number
            let line_num = expression.parse::<u64>().unwrap();
            return Some(Tag {
                name: name.to_string(),
                file: file.to_string(),
                expression: expression.to_string(),
                line_num: Some(line_num),
                tag_type,
            });
        }

        // default case, expression was not parsed correctly
        return Some(Tag {
            name: name.to_string(),
            file: file.to_string(),
            expression: expression.to_string(),
            line_num: None,
            tag_type,
        });
    }
}
