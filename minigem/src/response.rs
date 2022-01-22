use crate::{connection::GeminiStream, Error};
use std::io::{self, BufRead, BufReader, Read};
use std::str;

pub enum Status {
    Input = 10,
    SensitiveInput = 11,
    Success = 20,
    RedirectTemporary = 30,
    RedirectPermanent = 31,
    TemporaryFailure = 40,
    ServerUnavailable = 41,
    CGIError = 42,
    ProxyError = 43,
    SlowDown = 44,
    PermanentFailure = 50,
    NotFound = 51,
    Gone = 52,
    ProxyRequestRefused = 53,
    BadRequest = 59,
    ClientCertificateRequired = 60,
    CertificateNotAuthorised = 61,
    CertificateNotValid = 62,
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
            status: Status::Success,
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
    fn new(inner: R) -> Self {
        Self {
            inner,
            is_preformatting: false,
        }
    }

    fn read_line(&mut self) -> io::Result<Option<Line>> {
        let mut buf = Vec::new();

        self.inner.read_until(b'\n', &mut buf)?;

        let mut pos = 0;

        if buf.starts_with(b"```") {
            self.is_preformatting = !self.is_preformatting;
            Ok(Some(Line::PreformattingToggle))
        } else if self.is_preformatting {
            let start = pos;
            let end = next_line(&buf, &mut pos);

            Ok(Some(Line::PreformattedText(
                str::from_utf8(&buf[start..end]).unwrap().to_string(),
            )))
        } else if buf.starts_with(b"=>") {
            pos = 2;
            consume_whitespace(&buf, &mut pos);
            let link_start = pos;
            let link_end = next_whitespace(&buf, &mut pos);

            if let b'\r' | b'\n' = buf[pos] {
                next_line(&buf, &mut pos);
                return Ok(Some(Line::Link {
                    link: str::from_utf8(&buf[link_start..link_end])
                        .unwrap()
                        .to_string(),
                    name: None,
                }));
            }

            consume_whitespace(&buf, &mut pos);

            let name_start = pos;
            let name_end = next_line(&buf, &mut pos);

            return Ok(Some(Line::Link {
                link: str::from_utf8(&buf[link_start..link_end])
                    .unwrap()
                    .to_string(),
                name: Some(
                    str::from_utf8(&buf[name_start..name_end])
                        .unwrap()
                        .to_string(),
                ),
            }));
        } else if let Some(&b'#') = buf.get(pos) {
            let level = consume_header(&buf, &mut pos);
            consume_whitespace(&buf, &mut pos);
            let start = pos;
            let end = next_line(&buf, &mut pos);

            Ok(Some(Line::Heading {
                level,
                text: str::from_utf8(&buf[start..end]).unwrap().to_string(),
            }))
        } else if buf.starts_with(b"* ") {
            let start = 2;
            let end = next_line(&buf, &mut pos);

            Ok(Some(Line::UnorderedListItem(
                str::from_utf8(&buf[start..end]).unwrap().to_string(),
            )))
        } else if let Some(b'>') = buf.get(0) {
            let start = 1;
            let end = next_line(&buf, &mut pos);

            Ok(Some(Line::Quote(
                str::from_utf8(&buf[start..end]).unwrap().to_string(),
            )))
        } else {
            let start = pos;
            let end = next_line(&buf, &mut pos);

            Ok(Some(Line::Text(
                str::from_utf8(&buf[start..end]).unwrap().to_string(),
            )))
        }
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
fn next_whitespace(buf: &[u8], pos: &mut usize) -> usize {
    loop {
        match buf.get(*pos) {
            Some(b'\t' | b' ' | b'\r' | b'\n') => return *pos,
            Some(..) => *pos += 1,
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
            Ok(line) => line.map(Ok),
            Err(err) => Some(Err(err)),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    Text(String),
    Link { link: String, name: Option<String> },
    PreformattingToggle,
    PreformattedText(String),
    Heading { level: usize, text: String },
    UnorderedListItem(String),
    Quote(String),
}

#[cfg(test)]
mod test {
    use super::*;
    use io::Cursor;

    #[test]
    fn test_parse() {
        let input = "  normal text line here  \r\n\
### Heading 3\r\n\
```\r\n\
### Heading 3\r\n\
```\r\n\
=> gemini://example.org/\r\n\
=> gemini://example.org/ An example link\r\n\
=> gemini://example.org/foo	Another example link at the same host\r\n\
=> foo/bar/baz.txt	A relative link\r\n\
=> 	gopher://example.org:70/1 A gopher link\r\n\
\x20\x20another normal line here\r\n\
";
        let buf = Cursor::new(input);
        let mut lines = Lines::new(buf);

        let line = lines.next().unwrap().unwrap();
        assert_eq!(Line::Text("  normal text line here  ".to_string()), line);

        let line = lines.next().unwrap().unwrap();
        assert_eq!(
            Line::Heading {
                level: 3,
                text: "Heading 3".to_string(),
            },
            line
        );

        let line = lines.next().unwrap().unwrap();
        assert_eq!(Line::PreformattingToggle, line);

        let line = lines.next().unwrap().unwrap();
        assert_eq!(Line::PreformattedText("### Heading 3".to_string()), line);

        let line = lines.next().unwrap().unwrap();
        assert_eq!(Line::PreformattingToggle, line);

        let line = lines.next().unwrap().unwrap();
        assert_eq!(
            Line::Link {
                link: "gemini://example.org/".to_string(),
                name: None
            },
            line
        );

        let line = lines.next().unwrap().unwrap();
        assert_eq!(
            Line::Link {
                link: "gemini://example.org/".to_string(),
                name: Some("An example link".to_string())
            },
            line
        );

        let line = lines.next().unwrap().unwrap();
        assert_eq!(
            Line::Link {
                link: "gemini://example.org/foo".to_string(),
                name: Some("Another example link at the same host".to_string())
            },
            line
        );

        let line = lines.next().unwrap().unwrap();
        assert_eq!(
            Line::Link {
                link: "foo/bar/baz.txt".to_string(),
                name: Some("A relative link".to_string())
            },
            line
        );

        let line = lines.next().unwrap().unwrap();
        assert_eq!(
            Line::Link {
                link: "gopher://example.org:70/1".to_string(),
                name: Some("A gopher link".to_string())
            },
            line
        );

        let line = lines.next().unwrap().unwrap();
        assert_eq!(Line::Text("  another normal line here".to_string()), line);
    }
}
