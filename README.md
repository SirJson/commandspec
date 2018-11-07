# tb2f_commandspec

Like commandspec minus `failure`

* I removed failure from the dependencies because I don't see any reason why my application needs to indirectly depend on it as well. 
* I updated all other dependencies
* I simplified the error handling. `CommandError` implements now `std::fmt::Display` with the same messages + metadata so there should be no difference in information between the two libraries. It might now not be as fancy as failure but if you don't want to depend on `failure` here you go.

> tb2f means too big to fail.

> Also remember just because I prefer not to use `failure` doesn't mean the original comspec is bad. I provide just another option. 

That's it folks! I leave the old readme blow almost as it is because the API didn't change.

---

Simple Rust macro for building `std::process::Command` objects. Uses macro_rules! and works on stable.

```toml
[dependencies]
tb2f_commandspec = "0.12.2"
```

Then:

```rust
#[macro_use]
extern crate tb2f_commandspec;

use tb2f_commandspec::CommandSpec; // .execute() method on Command
use std::process::Command;

let result = execute!(
    r"
        cd path/location
        export RUST_LOG=full
        export RUST_BACKTRACE=1
        cargo run {release_flag} --bin {bin_name} -- {args}
    ",
    release_flag=Some("--release"),
    bin_name="binary",
    args=vec!["arg1", "arg2"],
)?;
// result = Ok(()) on success (error code 0), Err(CommandError) for all else
```

Format of the commandspec input, in order:

* (optional) `cd <path>` to set the current working directory of the command, where path can be a literal, a quoted string, or format variable.
* (optional) one or more `export <name>=<value>` lines to set environment variables, with the same formatting options.
* Last, a command you want to invoke, optionally with format arguments.

### Features:

* format-like invocation makes it easy to interpolate variables, with automatic quoting
* Equivalent syntax to shell when prototyping
* Works on stable Rust.
* `failure` free. - _Doesn't mean it won't fail or panic though :wink:_

## License

MIT or Apache-2.0, at your option.
