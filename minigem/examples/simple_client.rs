use minigem::{Lines, Request};
use std::error;
fn main() -> Result<(), Box<dyn error::Error>> {
    let res = Request::new("gemini://gemini.circumlunar.space").send()?;

    println!("status: {:?}", res.status);
    println!("meta: {:?}", res.meta);

    let lines = Lines::from(res.body);

    for line in lines {
        let line = line?;
        println!("{:?}", line);
    }

    // let mut cursor = Cursor::new(body);

    // cursor.set_position(0);

    // copy(&mut cursor, &mut stdout())?;

    Ok(())
}
