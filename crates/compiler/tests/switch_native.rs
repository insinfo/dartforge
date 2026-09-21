//! Contrato do pipeline completo para switch no alvo nativo LLVM.
use dartforge_compiler::compile_llvm;

/// O pipeline aceita switch instrução e expressão sobre os tipos do subconjunto nativo.
#[test]
fn llvm_pipeline_lowers_every_supported_switch_form() {
    for source in [
        // int com default e cases que encerram sem fallthrough.
        "void main(){switch(2){case 1: print(10); case 2: print(20); default: print(30);}}",
        // bool exaustivo, sem default.
        "void main(){switch(true){case true: print(1); case false: print(2);}}",
        // String usa comparação de conteúdo do runtime.
        "void main(){String s='b'; switch(s){case 'a': print(1); case 'b': print(2);}}",
        // enum simples compara os singletons canônicos.
        "enum C{red,green} void main(){C c=C.red; switch(c){case C.red: print(1); case C.green: print(2);}}",
        // binding tipado com guarda, avaliada apenas no braço selecionado.
        "void main(){switch(3){case int v when v>2: print(v); default: print(0);}}",
        // switch expressão com wildcard.
        "void main(){print(switch(2){1=>10,2=>20,_=>30});}",
        // padrão de objeto vazio e binding nominal sobre cone selado.
        "sealed class S{} class A extends S{} class B extends S{} void main(){S v=A(); print(switch(v){A a=>1,B()=>2});}",
        // discriminante nullable escolhe null antes de promover o payload.
        "void main(){int? n=1; switch(n){case null: print(0); case int v: print(v);}}",
        // break encerra o switch e continue permanece ligado ao laço externo.
        "void main(){for(var i=0;i<3;i++){switch(i){case 0: continue; case 1: break; default: print(i);} print(9);}}",
    ] {
        compile_llvm(source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
    }
}

/// O discriminante é avaliado uma única vez, antes de qualquer padrão ou guarda.
#[test]
fn llvm_pipeline_evaluates_the_scrutinee_once() {
    let ir = compile_llvm(
        "int pick(){print(0);return 2;} void main(){switch(pick()){case 1: print(10); case 2: print(20); default: print(30);}}",
    )
    .unwrap();
    assert_eq!(ir.matches("call i64 @df_fn_0()").count(), 1);
}

/// Recursos sem lowering dentro de padrões, guardas e corpos mantêm span e motivo.
#[test]
fn llvm_pipeline_keeps_specific_diagnostics_inside_switches() {
    for (source, fragment) in [
        (
            "void main(){int x=1; switch(x){case 1: print(x as int); default: print(2);}}",
            "x as int",
        ),
        (
            "void main(){int x=1; print(switch(x){1 when x is int=>1,_=>0});}",
            "x is int",
        ),
    ] {
        let error = compile_llvm(source).expect_err("lowering deve ser explícito");
        assert_eq!(
            error.message,
            "LLVM AOT ainda não suporta testes e casts de tipos reificados"
        );
        assert_eq!(&source[error.span.start..error.span.end], fragment);
    }
}
