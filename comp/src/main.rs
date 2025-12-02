use comp::Compiler;

fn main() {
    let mut compiler = Compiler::new();
    let file = compiler.add_file("comp/example/test.gba").unwrap();
    dbg!(compiler.bind(file));
    // compiler.compile(file);
    println!("Hello, world!");
}
