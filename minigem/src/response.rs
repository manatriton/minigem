use crate::Error;
use std::io::{self, BufRead, BufReader, Read};
use std::str;

macro_rules! slice_next_line {
    ($buf:expr, &mut $pos:ident) => {{
        let start = $pos;
        let end = next_line($buf, &mut $pos);
        Slice { start, end }
    }};
}

macro_rules! slice_next_whitespace {
    ($buf:expr, &mut $pos:ident) => {{
        let start = $pos;
        let end = next_whitespace($buf, &mut $pos);
        Slice { start, end }
    }};
}

#[derive(Debug, PartialEq, Eq)]
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

pub struct Response<R> {
    pub status: Status,
    pub meta: String,
    pub body: Body<BufReader<R>>,
}

pub struct Body<R> {
    inner: R,
}

impl<R: BufRead> Body<R> {
    fn new(inner: R) -> Self {
        Body { inner }
    }

    pub fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, io::Error> {
        self.inner.read_to_end(buf)
    }
}

macro_rules! next {
    ($buf:expr, &mut $pos:ident) => {{
        match $buf.get($pos) {
            Some(&b) => {
                $pos += 1;
                b
            }
            None => return Err(Error::UnexpectedEOF),
        }
    }};
    ($buf:expr, $pos:ident) => {{
        match $buf.get(*$pos) {
            Some(&b) => {
                *$pos += 1;
                b
            }
            None => return Err(Error::UnexpectedEOF),
        }
    }};
}

impl<R: Read> Response<R> {
    pub(crate) fn try_from_reader(rdr: R) -> Result<Self, Error> {
        let mut buf = String::new();
        let mut rdr = BufReader::new(rdr);

        rdr.read_line(&mut buf).unwrap();

        let buf = buf.as_bytes();
        let mut pos = 0;
        let status = parse_status(buf, &mut pos)?;

        if next!(buf, &mut pos) != b' ' {
            return Err(Error::BadHeader);
        }

        let slice = slice_next_line!(buf, &mut pos);
        let meta = unsafe { str::from_utf8_unchecked(&buf[slice.start..slice.end]).to_string() };

        Ok(Self {
            status,
            meta,
            body: Body::new(rdr),
        })
    }
}

#[inline]
fn parse_status(buf: &[u8], pos: &mut usize) -> Result<Status, Error> {
    let tens = next!(buf, pos);
    match tens {
        b'0'..=b'9' => {}
        _ => return Err(Error::BadHeader),
    }

    let ones = next!(buf, pos);
    match ones {
        b'0'..=b'9' => {}
        _ => return Err(Error::BadHeader),
    }

    let status = ((tens - b'0') * 10) + (ones - b'0');
    let status = match status {
        10 => Status::Input,
        11 => Status::SensitiveInput,
        20 => Status::Success,
        30 => Status::RedirectTemporary,
        31 => Status::RedirectPermanent,
        40 => Status::TemporaryFailure,
        41 => Status::ServerUnavailable,
        42 => Status::CGIError,
        43 => Status::ProxyError,
        44 => Status::SlowDown,
        50 => Status::PermanentFailure,
        51 => Status::NotFound,
        52 => Status::Gone,
        53 => Status::ProxyRequestRefused,
        59 => Status::BadRequest,
        60 => Status::ClientCertificateRequired,
        61 => Status::CertificateNotAuthorised,
        62 => Status::CertificateNotValid,
        _ => return Err(Error::BadHeader),
    };

    Ok(status)
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
        let mut src = String::new();

        self.inner.read_line(&mut src)?;

        let buf = src.as_bytes();

        if buf.is_empty() || buf[buf.len() - 1] != b'\n' {
            return Ok(None);
        }

        let mut pos = 0;

        if buf.starts_with(b"```") {
            self.is_preformatting = !self.is_preformatting;
            Ok(Some(Line {
                src,
                kind: LineKind::PreformattingToggle,
                link: None,
                text: None,
                level: None,
            }))
        } else if self.is_preformatting {
            let slice = slice_next_line!(buf, &mut pos);
            Ok(Some(Line {
                src,
                kind: LineKind::PreformattedText,
                text: Some(slice),
                link: None,
                level: None,
            }))
        } else if buf.starts_with(b"=>") {
            pos = 2;
            consume_whitespace(buf, &mut pos);
            let link_slice = slice_next_whitespace!(buf, &mut pos);
            if let b'\r' | b'\n' = buf[pos] {
                next_line(buf, &mut pos);

                Ok(Some(Line {
                    src,
                    kind: LineKind::Link,
                    link: Some(link_slice),
                    text: None,
                    level: None,
                }))
            } else {
                consume_whitespace(buf, &mut pos);

                let slice = slice_next_line!(buf, &mut pos);
                Ok(Some(Line {
                    src,
                    kind: LineKind::Link,
                    link: Some(link_slice),
                    text: Some(slice),
                    level: None,
                }))
            }
        } else if let Some(&b'#') = buf.get(pos) {
            let level = consume_header(buf, &mut pos);
            consume_whitespace(buf, &mut pos);
            let slice = slice_next_line!(buf, &mut pos);
            Ok(Some(Line {
                src,
                kind: LineKind::Heading,
                link: None,
                text: Some(slice),
                level: Some(level),
            }))
        } else if buf.starts_with(b"* ") {
            let slice = slice_next_line!(buf, &mut pos);
            Ok(Some(Line {
                src,
                kind: LineKind::UnorderedListItem,
                link: None,
                text: Some(slice),
                level: None,
            }))
        } else if let Some(b'>') = buf.get(0) {
            pos = 1;
            let slice = slice_next_line!(buf, &mut pos);
            Ok(Some(Line {
                src,
                kind: LineKind::Quote,
                link: None,
                text: Some(slice),
                level: None,
            }))
        } else {
            let slice = slice_next_line!(buf, &mut pos);
            Ok(Some(Line {
                src,
                kind: LineKind::Text,
                link: None,
                text: Some(slice),
                level: None,
            }))
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

impl<R: BufRead> From<Body<R>> for Lines<R> {
    fn from(body: Body<R>) -> Self {
        Self::new(body.inner)
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
pub struct Line {
    kind: LineKind,
    src: String,
    link: Option<Slice>,
    text: Option<Slice>,
    level: Option<usize>,
}

impl Line {
    #[inline]
    pub fn kind(&self) -> LineKind {
        self.kind
    }

    #[inline]
    pub fn text(&self) -> Option<&str> {
        self.text.map(|Slice { start, end }| unsafe {
            str::from_utf8_unchecked(&self.src.as_bytes()[start..end])
        })
    }

    #[inline]
    pub fn link(&self) -> Option<&str> {
        self.link.map(|Slice { start, end }| unsafe {
            str::from_utf8_unchecked(&self.src.as_bytes()[start..end])
        })
    }

    #[inline]
    pub fn level(&self) -> Option<usize> {
        self.level
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Slice {
    start: usize,
    end: usize,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LineKind {
    Text,
    Link,
    PreformattingToggle,
    PreformattedText,
    Heading,
    UnorderedListItem,
    Quote,
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
        let lines = Lines::new(buf);

        let lines: Vec<Line> = lines.map(|line| line.unwrap()).collect();

        assert_eq!(lines[0].kind, LineKind::Text);
        assert_eq!(lines[0].text().unwrap(), "  normal text line here  ");

        assert_eq!(lines[1].kind, LineKind::Heading);
        assert_eq!(lines[1].text().unwrap(), "Heading 3");
        assert_eq!(lines[1].level().unwrap(), 3);

        assert!(matches!(lines[2].kind, LineKind::PreformattingToggle));

        assert_eq!(lines[3].kind, LineKind::PreformattedText);
        assert_eq!(lines[3].text().unwrap(), "### Heading 3");

        assert!(matches!(lines[4].kind, LineKind::PreformattingToggle));

        assert_eq!(lines[5].kind, LineKind::Link);
        assert_eq!(lines[5].link().unwrap(), "gemini://example.org/");
        assert!(lines[5].text().is_none());

        assert_eq!(lines[6].kind, LineKind::Link);
        assert_eq!(lines[6].link().unwrap(), "gemini://example.org/");
        assert_eq!(lines[6].text().unwrap(), "An example link");

        assert_eq!(lines[7].kind, LineKind::Link);
        assert_eq!(lines[7].link().unwrap(), "gemini://example.org/foo");
        assert_eq!(
            lines[7].text().unwrap(),
            "Another example link at the same host"
        );

        assert_eq!(lines[8].kind, LineKind::Link);
        assert_eq!(lines[8].link().unwrap(), "foo/bar/baz.txt");
        assert_eq!(lines[8].text().unwrap(), "A relative link");

        assert_eq!(lines[9].kind, LineKind::Link);
        assert_eq!(lines[9].link().unwrap(), "gopher://example.org:70/1");
        assert_eq!(lines[9].text().unwrap(), "A gopher link");

        assert_eq!(lines[10].kind, LineKind::Text);
        assert_eq!(lines[10].text().unwrap(), "  another normal line here");
    }
}
