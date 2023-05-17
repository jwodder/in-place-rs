[![Project Status: WIP – Initial development is in progress, but there has not yet been a stable, usable release suitable for the public.](https://www.repostatus.org/badges/latest/wip.svg)](https://www.repostatus.org/#wip)
[![CI Status](https://github.com/jwodder/in-place-rs/actions/workflows/test.yml/badge.svg)](https://github.com/jwodder/in-place-rs/actions/workflows/test.yml)
[![codecov.io](https://codecov.io/gh/jwodder/in-place-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/jwodder/in-place-rs)
[![Minimum Supported Rust Version](https://img.shields.io/badge/MSRV-1.65-orange)](https://www.rust-lang.org)
[![MIT License](https://img.shields.io/github/license/jwodder/in-place-rs.svg)](https://opensource.org/licenses/MIT)

[GitHub](https://github.com/jwodder/in-place-rs) | [Issues](https://github.com/jwodder/in-place-rs/issues)

The `in_place` library provides an `InPlace` type for reading & writing a file
"in-place": data that you write ends up at the same filepath that you read
from, and `in_place` takes care of all the necessary mucking about with
temporary files for you.

For example, given the file `somefile.txt`:

```text
'Twas brillig, and the slithy toves
    Did gyre and gimble in the wabe;
All mimsy were the borogoves,
    And the mome raths outgrabe.
```

and the following program:

```rust
use in_place::InPlace;
use std::io::{BufRead, BufReader, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let inp = InPlace::new("somefile.txt").open()?;
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
```

after running the program, `somefile.txt` will have been edited in place,
reducing it to just:

```text
'Tws brllg, nd th slthy tvs
    Dd gyr nd gmbl n th wb;
ll mmsy wr th brgvs,
    nd th mm rths tgrb.
```

and no sign of those pesky vowels remains!  If you want a sign of those pesky
vowels to remain, you can instead save the file's original contents in, say,
`somefile.txt~` by opening the file with:

```rust
let inp = InPlace::new("somefile.txt")
    .backup(in_place::Backup::Append("~".into())
    .open()?;
```

or save to `someotherfile.txt` with:

```rust
let inp = InPlace::new("somefile.txt")
    .backup(in_place::Backup::Path("someotherfile.txt".into())
    .open()?;
```
