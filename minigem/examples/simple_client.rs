use minigem::Request;
use std::borrow::Borrow;
use std::error;
use std::io::{copy, stdout, Cursor};
fn main() -> Result<(), Box<dyn error::Error>> {
    let mut res = Request::new("gemini://gemini.circumlunar.space/").send()?;
    let mut body = Vec::new();

    res.body.read_to_end(&mut body)?;

    let mut cursor = Cursor::new(body);

    cursor.set_position(0);

    copy(&mut cursor, &mut stdout())?;

    Ok(())
}
