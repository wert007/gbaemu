use std::io::stdout;

use comp::{Compiler, HasLocation};

fn main() {
    let mut compiler = Compiler::new();
    let file = compiler.add_file("comp/example/test.gba").unwrap();
    // for token in compiler.lex(file) {
    //     eprintln!("{:?}: {}", token.kind, &compiler[token.location()]);
    // }
    compiler.bind(file);
    compiler.write_diagnostics(&mut stdout()).unwrap();
    // compiler.compile(file);
    println!("Hello, world!");
}
