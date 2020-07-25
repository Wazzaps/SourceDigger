use std::fs::File;
use std::io::{BufRead, BufReader, Read, Split};
use std::path::PathBuf;
use std::iter::Enumerate;

#[cfg(test)]
mod tests {
    use crate::{Ctags, Symbol, SymbolType};
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
            Some(Symbol {
                name: "Hello".into(),
                file: "example.c".into(),
                expression: "/^int Hello() {$/".into(),
                line_num: Some(3),
                symbol_type: SymbolType::Function
            })
        );
        assert_eq!(
            tags.next(),
            Some(Symbol {
                name: "World".into(),
                file: "example.c".into(),
                expression: "/^int World() {$/".into(),
                line_num: Some(6),
                symbol_type: SymbolType::Function
            })
        );
        assert_eq!(
            tags.next(),
            Some(Symbol {
                name: "FOOBAR".into(),
                file: "example.c".into(),
                expression: "1".into(),
                line_num: Some(1),
                symbol_type: SymbolType::Define
            })
        );
    }
}

pub struct Ctags<R: Read> {
    source: BufReader<R>,
    code_dir: Option<PathBuf>,
    current_file: Option<(PathBuf, Enumerate<Split<BufReader<File>>>)>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub file: String,
    pub expression: String,
    pub line_num: Option<u64>,
    pub symbol_type: SymbolType,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum SymbolType {
    Unknown = 0,
    Function = 1,
    Define = 2,
}

impl<R: Read> Ctags<R> {
    pub fn new<P: Into<PathBuf>>(ctags_contents: R, code_dir: Option<P>) -> Self {
        Self {
            source: BufReader::new(ctags_contents),
            code_dir: code_dir.map(|c| c.into()),
            current_file: None,
        }
    }
}

impl<R: Read> Iterator for Ctags<R> {
    type Item = Symbol;

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

        let symbol_type = match extra.next().unwrap_or("") {
            "f" => SymbolType::Function,
            "d" => SymbolType::Define,
            _ => SymbolType::Unknown,
        };

        if expression.starts_with("/^") && expression.ends_with("$/") {
            // regex match
            // TODO: Use regex, not string matching
            if let Some(code_dir) = &self.code_dir {
                let code_path = code_dir.join(file);
                if self.current_file.is_none() || code_path != self.current_file.as_ref().unwrap().0 {
                    self.current_file.replace((
                        code_path.clone(),
                        BufReader::new(File::open(&code_path).unwrap()).split('\n' as u8).enumerate()
                    ));
                }

                // Read file
                let line_num =
                    self.current_file.as_mut().unwrap().1
                        .find_map(|(line, contents)| {
                            if contents.unwrap() == expression[2..expression.len() - 2].as_bytes() {
                                Some((line as u64) + 1)
                            } else {
                                None
                            }
                        });

                return Some(Symbol {
                    name: name.to_string(),
                    file: file.to_string(),
                    expression: expression.to_string(),
                    line_num,
                    symbol_type,
                });
            }
        } else if expression.chars().all(char::is_numeric) {
            // explicit line number
            let line_num = expression.parse::<u64>().unwrap();
            return Some(Symbol {
                name: name.to_string(),
                file: file.to_string(),
                expression: expression.to_string(),
                line_num: Some(line_num),
                symbol_type,
            });
        }

        // default case, expression was not parsed correctly
        return Some(Symbol {
            name: name.to_string(),
            file: file.to_string(),
            expression: expression.to_string(),
            line_num: None,
            symbol_type,
        });
    }
}
