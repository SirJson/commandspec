extern crate shlex;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

#[cfg(windows)]
extern crate kernel32;
#[cfg(unix)]
extern crate nix;
#[cfg(windows)]
extern crate winapi;

use std::process::Command;
use std::fmt;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::path::{Path, PathBuf};


pub mod macros;
mod process;
mod signal;

use process::Process;
use signal::Signal;

lazy_static! {
    static ref PID_MAP: Arc<Mutex<HashMap<i32, Process>>> = Arc::new(Mutex::new(HashMap::new()));
}

// This is basically what failure does but without bail!
macro_rules! check {
    ($cond:expr, $e:expr) => {
        if !($cond) {
            return Err($e);
        }
    };
}

pub fn disable_cleanup_on_ctrlc() {
    signal::uninstall_handler();
}

pub fn cleanup_on_ctrlc() {
    signal::install_handler(move |sig: Signal| {
        match sig {
            // SIGCHLD is special, initiate reap()
            Signal::SIGCHLD => {
                for (_pid, process) in PID_MAP.lock().unwrap().iter() {
                    process.reap();
                }
            }
            Signal::SIGINT => {
                for (_pid, process) in PID_MAP.lock().unwrap().iter() {
                    process.signal(sig);
                }
                ::std::process::exit(130);
            }
            _ => {
                for (_pid, process) in PID_MAP.lock().unwrap().iter() {
                    process.signal(sig);
                }
            }
        }
    });
}

pub struct SpawnGuard(i32);

impl ::std::ops::Drop for SpawnGuard {
    fn drop(&mut self) {
        if let Some(process) = PID_MAP.lock().unwrap().remove(&self.0) { process.reap() }
    }
}

//---------------

pub trait CommandSpecExt {
    fn execute(self) -> Result<(), CommandError>;

    fn scoped_spawn(self) -> Result<SpawnGuard, ::std::io::Error>;
}

#[derive(Debug)]
pub enum CommandError {
    Io(::std::io::Error),
    Interrupt,
    Code(i32),
    TooManyCDArgs(usize,usize),
    NotEnoughExportArgs(usize,usize),
    NoChangeDir,
    InvalidExport,
    ExportMispositioned,
    NoCommand
}

impl std::fmt::Display for CommandError
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        match self {
            CommandError::Io(err) => write!(f,"{}",format_args!("Encountered an IO error: {:?}",err)),
            CommandError::Interrupt => write!(f, "Command was interrupted."),
            CommandError::Code(code) => write!(f, "{}",format_args!("Command failed with error code {}",code)),
            CommandError::TooManyCDArgs(expected,found) => write!(f, "{}",format_args!("Too many arguments in cd; expected {}, found {}",expected,found)),
            CommandError::NotEnoughExportArgs(expected,found) => write!(f, "{}",format_args!("Not enough arguments in export; expected at least {}, found {}",expected,found)),
            CommandError::NoChangeDir => write!(f, "cd should be the first line in your command! macro."),
            CommandError::InvalidExport => write!(f, "Expected export of the format NAME=VALUE"),
            CommandError::ExportMispositioned => write!(f, "exports should follow cd but precede your command in the command! macro."),
            CommandError::NoCommand => write!(f, "Didn't find a command in your command! macro."),
        }
    }
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
    fn execute(mut self) -> Result<(), CommandError> {
        match self.spawn() {
            Ok(mut child) => {
                match child.wait() {
                    Ok(status) => {
                        if status.success() {
                            Ok(())
                        } else if let Some(code) = status.code() {
                            Err(CommandError::Code(code))
                        } else {
                            Err(CommandError::Interrupt)
                        }
                    }
                    Err(err) => {
                        Err(CommandError::Io(err))
                    }
                }
            },
            Err(err) => Err(CommandError::Io(err)),
        }
    }

    fn scoped_spawn(self) -> Result<SpawnGuard, ::std::io::Error> {
        let process = Process::new(self)?;
        let id = process.id();
        PID_MAP.lock().unwrap().insert(id, process);
        Ok(SpawnGuard(id))
    }
}

//---------------

pub enum CommandArg {
    Empty,
    Literal(String),
    List(Vec<String>),
}

fn shell_quote(value: &str) -> String {
    shlex::quote(value).to_string()
}

impl fmt::Display for CommandArg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use CommandArg::*;
        match *self {
            Empty => write!(f, ""),
            Literal(ref value) => {
                write!(f, "{}", shell_quote(value))
            },
            List(ref list) => {
                write!(f, "{}", list
                    .iter()
                    .map(|x| shell_quote(x).to_string())
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
        let cd = if let Some(ref cd) = self.cd {
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

        // On windows, we run in cmd.exe by default. (This code is a naive way
        // of accomplishing this and may contain errors.)
        if cfg!(windows) {
            let mut cmd = Command::new("cmd");
            cmd.current_dir(cd);
            let invoke_string = format!("{} {}", binary.as_path().to_string_lossy(), self.args.join(" "));
            cmd.args(&["/C", &invoke_string]);
            for (key, value) in &self.env {
                cmd.env(key, value);
            }
            return cmd;
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
#[cfg(windows)]
fn canonicalize_path<'p, P>(path: P) -> Result<PathBuf, Error>
where P: Into<&'p Path> {
    use std::ffi::OsString;
    use std::os::windows::prelude::*;

    let canonical = path.into().canonicalize()?;
    let vec_chars = canonical.as_os_str().encode_wide().collect::<Vec<u16>>();
    if vec_chars[0..4] == [92, 92, 63, 92] {
        return Ok(Path::new(&OsString::from_wide(&vec_chars[4..])).to_owned());
    }

    Ok(canonical)
}

#[cfg(not(windows))]
fn canonicalize_path<'p, P>(path: P) -> Result<PathBuf, CommandError>
where P: Into<&'p Path> {
    Ok(path.into().canonicalize().map_err(CommandError::Io)?)
}

//---------------

pub fn commandify(value: &str) -> Result<Command, CommandError> {
    let lines = value.trim().split('\n').map(String::from).collect::<Vec<_>>();

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
        let mut line = shlex::split(&raw_line).unwrap_or_default();
        if state == SpecState::Cmd {
            command_lines.push(raw_line);
        } else {
            if raw_line.trim().is_empty() {
                continue;
            }

            match line.get(0).map(|x| x.as_ref()) {
                Some("cd") => {
                    if state != SpecState::Cd {
                        return Err(CommandError::NoChangeDir);
                    }
                    check!(line.len() == 2, CommandError::TooManyCDArgs(1,line.len() - 1));
                    cd = Some(line.remove(1));
                    state = SpecState::Env;
                }
                Some("export") => {
                    if state != SpecState::Cd && state != SpecState::Env {
                        return Err(CommandError::ExportMispositioned);
                    }
                    check!(line.len() >= 2, CommandError::NotEnoughExportArgs(1,line.len() - 1));
                    for item in &line[1..] {
                        let mut items = item.splitn(2, '=').collect::<Vec<_>>();
                        check!(!items.is_empty(), CommandError::InvalidExport);
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
        return Err(CommandError::NoCommand);
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
