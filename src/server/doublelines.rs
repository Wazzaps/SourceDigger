#[derive(Debug)]
pub struct DoubleLines<B> {
    source: B,
    buf: String,
}

/// Splits string at "\n\n" segments
impl<B: BufRead> Iterator for DoubleLines<B> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Result<String>> {
        // let mut buf = String::new();
        match self.source.read_line(&mut self.buf) {
            Ok(0) => None,
            Ok(_n) => {
                if self.buf.ends_with('\n') {
                    self.buf.pop();
                    if self.buf.ends_with('\r') {
                        self.buf.pop();
                    }
                }
                Some(Ok(self.buf))
            }
            Err(e) => Some(Err(e)),
        }
    }
}