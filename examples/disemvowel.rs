use in_place::InPlace;
use std::io::{BufRead, BufReader, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or_else(|| "no path argument supplied".to_string())?;
    let inp = InPlace::new(path).open()?;
    let reader = BufReader::new(inp.reader());
    let mut writer = inp.writer();
    for line in reader.lines() {
        let mut line = line?;
        line.retain(|ch| !"AEIOUaeiou".contains(ch));
        writeln!(writer, "{line}")?;
    }
    inp.save()?;
    Ok(())
}
