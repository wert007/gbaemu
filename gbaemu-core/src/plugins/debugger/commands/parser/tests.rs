use super::*;
use crate::plugins::debugger::commands::Command;
fn p(t: &str) -> Result<Command, ()> {
    parse_command(t.to_string())
}

#[test]
fn parse_tests() {
    expect_echo(p("123"), "123");
    expect_read_memory(p("[123]"), 123);
    assert_eq!(p("b"), Ok(Command::GoBack));
}

fn expect_echo(p: Result<Command, ()>, arg: &str) {
    let p = p.unwrap();
    let e = match p {
        Command::Echo(e) => e,
        c => panic!("Command {c:?} was not echo!"),
    };
    assert_eq!(e, arg);
}

fn expect_read_memory(p: Result<Command, ()>, arg: u32) {
    let p = p.unwrap();
    let e = match p {
        Command::ReadMemory(e) => e,
        c => panic!("Command {c:?} was not echo!"),
    };
    match e {
        Address::Indirect(u) | Address::Literal(u) => assert_eq!(u, arg),
        Address::Register(register_index) => todo!(),
    }
}
