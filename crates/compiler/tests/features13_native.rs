//! Contrato explícito entre os novos recursos do frontend e o subconjunto LLVM.
use dartforge_compiler::compile_llvm;

/// Recursos sem lowering não podem ser emitidos silenciosamente como outro programa.
#[test]
fn llvm_rejects_unimplemented_features13() {
    for source in [
        "T id<T>(T x) => x; void main(){print(id<int>(1));}",
        "enum E { a(1); final int n; const E(this.n); } void main(){print(E.a.n);}",
        "void main(){print(switch(true){true=>1,false=>2});}",
        "void main(){switch(true){case true: print(1); case false: print(2);}}",
    ] {
        let error = compile_llvm(source).expect_err("lowering deve ser explícito");
        assert!(error.message.contains("LLVM"), "{error:?}");
    }
}

/// Constantes escalares locais não precisam de objetos canônicos no runtime.
#[test]
fn llvm_accepts_scalar_local_constants() {
    compile_llvm("void main(){const int n=1+2;const String s='a'+'b';print(n);print(s);}").unwrap();
}
