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
