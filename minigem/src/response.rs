use crate::{connection::GeminiStream, Error};
use std::io::{self, BufRead, BufReader, Read};
use std::str;

pub enum Status {
    Ok = 20,
}

pub struct Response {
    pub status: Status,
    pub meta: String,
    pub body: Body,
}

pub struct Body {
    inner: BufReader<GeminiStream>,
}

impl Body {
    fn new(inner: BufReader<GeminiStream>) -> Self {
        Body { inner }
    }
}

impl Body {
    pub fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, io::Error> {
        self.inner.read_to_end(buf)
    }
}

impl Response {
    pub(crate) fn try_from_stream(stream: GeminiStream) -> Result<Self, Error> {
        let mut buf = String::new();
        let mut rdr = BufReader::new(stream);

        rdr.read_line(&mut buf).unwrap();

        Ok(Self {
            status: Status::Ok,
            meta: "".to_string(),
            body: Body::new(rdr),
        })
    }
}

pub struct Lines<R> {
    inner: R,
    is_preformatting: bool,
}

impl<R> Lines<R>
where
    R: BufRead,
{
    fn read_line(&mut self) -> io::Result<Option<Line>> {
        let mut buf = Vec::new();

        self.inner.read_until(b'\n', &mut buf)?;

        let mut pos = 0;

        if buf.starts_with(b"```") {
            self.is_preformatting = !self.is_preformatting;
            return Ok(Some(Line::PreformattingToggle));
        } else if self.is_preformatting {
            let start = pos;
            let end = next_line(&buf, &mut pos);

            return Ok(Some(Line::PreformattedText(
                str::from_utf8(&buf[start..end]).unwrap().to_string(),
            )));
        } else if buf.starts_with(b"=>") {
            pos = 2;
            consume_whitespace(&buf, &mut pos);

            // Move to newline e.g. with newline macro
        } else if let Some(&b'#') = buf.get(pos) {
            let level = consume_header(&buf, &mut pos);
            consume_whitespace(&buf, &mut pos);
            let start = pos;
            let end = next_line(&buf, &mut pos);

            return Ok(Some(Line::Heading {
                level: level,
                text: str::from_utf8(&buf[start..end]).unwrap().to_string(),
            }));
        } else {
            // Move to new line. Return string.
        }

        todo!()
    }
}

#[inline]
fn next_line(buf: &[u8], pos: &mut usize) -> usize {
    loop {
        match buf.get(*pos) {
            Some(&b) => match b {
                b'\r' => {
                    *pos += 1;
                    if let Some(&b'\n') = buf.get(*pos) {
                        *pos += 1;
                        return *pos - 2;
                    } else {
                        return *pos - 1;
                    }
                }
                b'\n' => {
                    *pos += 1;
                    return *pos - 1;
                }
                _ => {
                    *pos += 1;
                }
            },
            None => return *pos,
        }
    }
}

#[inline]
fn consume_header(buf: &[u8], pos: &mut usize) -> usize {
    let mut count = 0;
    while let Some(b'#') = buf.get(*pos) {
        *pos += 1;
        count += 1;
    }

    count
}

#[inline]
fn consume_whitespace(buf: &[u8], pos: &mut usize) {
    while let Some(b'\t' | b' ') = buf.get(*pos) {
        *pos += 1;
    }
}

impl<R> Iterator for Lines<R>
where
    R: BufRead,
{
    type Item = io::Result<Line>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_line() {
            Ok(None) => None,
            Ok(line) => match line {
                Some(line) => Some(Ok(line)),
                None => None,
            },
            Err(err) => Some(Err(err)),
        }
    }
}

pub enum Line {
    Text(String),
    Link(String),
    PreformattingToggle,
    PreformattedText(String),
    Heading { level: usize, text: String },
    UnorderedListItem(String),
    Quote(String),
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse() {}
}
