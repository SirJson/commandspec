extern crate shlex;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
extern crate ctrlc;
#[cfg(not(windows))]
extern crate libc;

use std::process::{Command, Child};
use std::fmt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use std::os::unix::process::CommandExt;
#[cfg(not(windows))]
use libc::{kill, SIGINT, setpgid};

// Re-export for macros.
pub use failure::Error;

pub mod macros;

#[cfg(not(windows))]
lazy_static! {
    static ref PID_MAP: Arc<Mutex<HashSet<i32>>> = Arc::new(Mutex::new(HashSet::new()));
}

#[cfg(not(windows))]
pub fn cleanup_on_ctrlc() {
    ctrlc::set_handler(|| {
        for pid in PID_MAP.lock().unwrap().iter() {
            unsafe {
                let _ = kill(-(*pid), SIGINT);
            }
        }
        ::std::process::exit(130);
    }).expect("Error setting Ctrl-C handler");
}

#[cfg(windows)]
pub fn cleanup_on_ctrlc() {
    // TODO this is a no-op until job control is implemented.
    // See https://github.com/rust-lang/cargo/blob/master/src/cargo/util/job.rs
}

pub struct SpawnGuard(pub Child);

impl SpawnGuard {
    #[cfg(not(windows))]
    fn abandon(self) {
        let id = self.0.id() as i32;
        PID_MAP.lock().unwrap().remove(&id);
        ::std::mem::forget(self);
    }

    #[cfg(windows)]
    fn abandon(self) {
        ::std::mem::forget(self);
    }
}

impl ::std::ops::Deref for SpawnGuard {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.0
    }
}

impl ::std::ops::DerefMut for SpawnGuard {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

#[cfg(not(windows))]
impl ::std::ops::Drop for SpawnGuard {
    fn drop(&mut self) {
        unsafe {
            let id = self.0.id() as i32;
            let _ = kill(-id, SIGINT);
            PID_MAP.lock().unwrap().remove(&id);
        }
    }
}

#[cfg(windows)]
impl ::std::ops::Drop for SpawnGuard {
    fn drop(&mut self) {
        self.kill();
    }
}

//---------------

pub trait CommandSpecExt {
    fn execute(&mut self) -> Result<(), CommandError>;

    fn scoped_spawn(&mut self) -> Result<SpawnGuard, ::std::io::Error>;
}

#[derive(Debug, Fail)]
pub enum CommandError {
    #[fail(display = "Encountered an IO error: {:?}", _0)]
    Io(#[cause] ::std::io::Error),

    #[fail(display = "Command was interrupted.")]
    Interrupt,

    #[fail(display = "Command failed with error code {}.", _0)]
    Code(i32),
}

impl CommandError {
    /// Returns the error code this command failed with. Can panic if not a `Code`.
    pub fn error_code(&self) -> i32 {
        if let CommandError::Code(value) = *self {
            value
        } else {
            panic!("Called error_code on a value that was not a CommandError::Code")
        }
    }
}

impl CommandSpecExt for Command {
    // Executes the command, and returns a versatile error struct
    fn execute(&mut self) -> Result<(), CommandError> {
        match self.scoped_spawn() {
            Ok(mut child) => {
                match child.wait() {
                    Ok(status) => {
                        let ret = if status.success() {
                            Ok(())
                        } else if let Some(code) = status.code() {
                            Err(CommandError::Code(code))
                        } else {
                            Err(CommandError::Interrupt)
                        };

                        child.abandon();

                        ret
                    }
                    Err(err) => {
                        Err(CommandError::Io(err))
                    }
                }
            },
            Err(err) => Err(CommandError::Io(err)),
        }
    }

    #[cfg(not(windows))]
    fn scoped_spawn(&mut self) -> Result<SpawnGuard, ::std::io::Error> {
        let child = self.before_exec(|| {
            unsafe {
                // Become this process' own leader.
                let _ = setpgid(0, 0);
            }
            Ok(())
        }).spawn()?;

        // TODO only add to PID_MAP if successful at becoming a process leader
        PID_MAP.lock().unwrap().insert(child.id() as i32);

        Ok(SpawnGuard(child))
    }

    #[cfg(windows)]
    fn scoped_spawn(&mut self) -> Result<SpawnGuard, ::std::io::Error> {
        // TODO enable job control on this to kill it?
        Ok(SpawnGuard(self.spawn()?))
    }
}

//---------------

pub enum CommandArg {
    Empty,
    Literal(String),
    List(Vec<String>),
}

fn shell_quote(value: &str) -> String {
    shlex::quote(&format!("{}", value)).to_string()
}

impl fmt::Display for CommandArg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use CommandArg::*;
        match *self {
            Empty => write!(f, ""),
            Literal(ref value) => {
                write!(f, "{}", shell_quote(&format!("{}", value)))
            },
            List(ref list) => {
                write!(f, "{}", list
                    .iter()
                    .map(|x| shell_quote(&format!("{}", x)).to_string())
                    .collect::<Vec<_>>()
                    .join(" "))
            }
        }
    }
}

impl<'a, 'b> From<&'a &'b str> for CommandArg {
    fn from(value: &&str) -> Self {
        CommandArg::Literal(value.to_string())
    }
}

impl From<String> for CommandArg {
    fn from(value: String) -> Self {
        CommandArg::Literal(value)
    }
}

impl<'a> From<&'a String> for CommandArg {
    fn from(value: &String) -> Self {
        CommandArg::Literal(value.to_string())
    }
}


impl<'a> From<&'a str> for CommandArg {
    fn from(value: &str) -> Self {
        CommandArg::Literal(value.to_string())
    }
}

impl<'a> From<&'a u64> for CommandArg {
    fn from(value: &u64) -> Self {
        CommandArg::Literal(value.to_string())
    }
}

impl<'a> From<&'a f64> for CommandArg {
    fn from(value: &f64) -> Self {
        CommandArg::Literal(value.to_string())
    }
}

impl<'a> From<&'a i32> for CommandArg {
    fn from(value: &i32) -> Self {
        CommandArg::Literal(value.to_string())
    }
}

impl<'a> From<&'a i64> for CommandArg {
    fn from(value: &i64) -> Self {
        CommandArg::Literal(value.to_string())
    }
}

impl<'a, T> From<&'a [T]> for CommandArg
    where T: fmt::Display {
    fn from(list: &[T]) -> Self {
        CommandArg::List(
            list
                .iter()
                .map(|x| format!("{}", x))
                .collect()
        )
    }
}

impl<'a, T> From<&'a Vec<T>> for CommandArg
    where T: fmt::Display {
    fn from(list: &Vec<T>) -> Self {
        CommandArg::from(list.as_slice())
    }
}

impl<'a, T> From<&'a Option<T>> for CommandArg
    where T: fmt::Display {
    fn from(opt: &Option<T>) -> Self {
        if let Some(ref value) = *opt {
            CommandArg::Literal(format!("{}", value))
        } else {
            CommandArg::Empty
        }
    }
}

pub fn command_arg<'a, T>(value: &'a T) -> CommandArg
    where CommandArg: std::convert::From<&'a T> {
    CommandArg::from(value)
}

//---------------

/// Represents the invocation specification used to generate a Command.
#[derive(Debug)]
struct CommandSpec {
    binary: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    cd: Option<String>,
}

impl CommandSpec {
    fn to_command(&self) -> Command {
        let mut cd = if let Some(ref cd) = self.cd {
            canonicalize_path(Path::new(cd)).unwrap()
        } else {
            ::std::env::current_dir().unwrap()
        };
        let mut binary = Path::new(&self.binary).to_owned();

        // On Windows, current_dir takes place after binary name resolution.
        // If current_dir is specified and the binary is referenced by a relative path,
        // add the dir change to its relative path.
        // https://github.com/rust-lang/rust/issues/37868
        if cfg!(windows) && binary.is_relative() && binary.components().count() != 1 {
            binary = cd.join(&binary);
        }

        let mut cmd = Command::new(binary);
        cmd.current_dir(cd);
        cmd.args(&self.args);
        for (key, value) in &self.env {
            cmd.env(key, value);
        }
        cmd
    }
}

// Strips UNC from canonicalized paths.
// See https://github.com/rust-lang/rust/issues/42869 for why this is needed.
fn canonicalize_path<'p, P>(path: P) -> Result<PathBuf, Error>
where P: Into<&'p Path> {
    let canonical = path.into().canonicalize()?;
    if cfg!(windows) {
        use std::ffi::OsString;
        use std::os::windows::prelude::*;
        let vec_chars = canonical.as_os_str().encode_wide().collect::<Vec<u16>>();
        if vec_chars[0..4] == [92, 92, 63, 92] {
            return Ok(Path::new(&OsString::from_wide(&vec_chars[4..])).to_owned());
        }
    }
    Ok(canonical)
}

//---------------

pub fn commandify(value: String) -> Result<Command, Error> {
    let lines = value.trim().split("\n").map(String::from).collect::<Vec<_>>();

    #[derive(Debug, PartialEq)]
    enum SpecState {
        Cd,
        Env,
        Cmd,
    }

    let mut env = HashMap::<String, String>::new();
    let mut cd = None;

    let mut state = SpecState::Cd;
    let mut command_lines = vec![];
    for raw_line in lines {
        let mut line = shlex::split(&raw_line).unwrap_or(vec![]);
        if state == SpecState::Cmd {
            command_lines.push(raw_line);
        } else {
            if raw_line.trim().is_empty() {
                continue;
            }

            match line.get(0).map(|x| x.as_ref()) {
                Some("cd") => {
                    if state != SpecState::Cd {
                        bail!("cd should be the first line in your command! macro.");
                    }
                    ensure!(line.len() == 2, "Too many arguments in cd; expected 1, found {}", line.len() - 1);
                    cd = Some(line.remove(1));
                    state = SpecState::Env;
                }
                Some("export") => {
                    if state != SpecState::Cd && state != SpecState::Env {
                        bail!("exports should follow cd but precede your command in the command! macro.");
                    }
                    ensure!(line.len() >= 2, "Not enough arguments in export; expected at least 1, found {}", line.len() - 1);
                    for item in &line[1..] {
                        let mut items = item.splitn(2, "=").collect::<Vec<_>>();
                        ensure!(items.len() > 0, "Expected export of the format NAME=VALUE");
                        env.insert(items[0].to_string(), items[1].to_string());
                    }
                    state = SpecState::Env;
                }
                None | Some(_) => {
                    command_lines.push(raw_line);
                    state = SpecState::Cmd;
                }
            }
        }
    }
    if state != SpecState::Cmd || command_lines.is_empty() {
        bail!("Didn't find a command in your command! macro.");
    }

    // Join the command string and split out binary / args.
    let command_string = command_lines.join("\n").replace("\\\n", "\n");
    let mut command = shlex::split(&command_string).expect("Command string couldn't be parsed by shlex");
    let binary = command.remove(0); 
    let args = command;

    // Generate the CommandSpec struct.
    let spec = CommandSpec {
        binary,
        args,
        env,
        cd,
    };

    // DEBUG
    // eprintln!("COMMAND: {:?}", spec);

    Ok(spec.to_command())
}
