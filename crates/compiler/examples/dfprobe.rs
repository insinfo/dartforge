//! Sonda temporária de diagnóstico.
fn main() {
    let src = "void main() { print(identical(7, 8)); }";
    let tokens = dartforge_lexer::lex(src).unwrap();
    let program = dartforge_parser::parse(&tokens, src.len()).unwrap();
    println!("{:?}", program.statements);
    println!("{:?}", dartforge_semantic::analyze(&program).err().map(|e| e.message));
}
