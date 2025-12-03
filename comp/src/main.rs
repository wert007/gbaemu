use std::io::stdout;

use comp::Compiler;

fn main() {
    let mut compiler = Compiler::new();
    let file = compiler.add_file("comp/example/test.gba").unwrap();
    // dbg!(compiler.parse(file));
    dbg!(compiler.bind(file));
    compiler.write_diagnostics(&mut stdout()).unwrap();
    // compiler.compile(file);
    println!("Hello, world!");
}
