//! Arquivo temporário de exploração; removido antes do fim do trabalho.
#[test]
fn explora() {
    println!("{}", dartforge_compiler::compile_llvm("int nested(bool c1, bool c2) => c1 ? (c2 ? 1 : 2) : 3; int nested_else(bool c1, bool c2) => c1 ? 1 : (c2 ? 2 : 3); void main(){print(nested(true, false));print(nested_else(false, true));}").unwrap());
}
