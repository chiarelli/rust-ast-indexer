use crate::application::protocol::{Command, Event};

pub fn write_event(e: &Event) {
    let s = serde_json::to_string(e).unwrap_or_else(|_| "{}".into());
    println!("{}", s);
}

pub fn read_command(line: &str) -> Option<Command> {
    serde_json::from_str(line).ok()
}
