//! Records: identidade estrutural, inferência e declaração desestruturante segura.

/// Analisa fonte real para exercitar o contrato entre parser e semântica.
fn check(source: &str) -> Result<dartforge_syntax::Resolution, String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::analyze(&program).map_err(|e| e.message)
}

/// Nomes canônicos preservam covariância, contextos e substituições genéricas.
#[test]
fn records_type_fields_context_and_inference() {
    for source in [
        "void main(){var r=(1,z:'s',true,a:2);print(r.$1);print(r.$2);print(r.a);print(r);}",
        "void main(){({int a,String z}) r=(z:'s',a:1);({Object a,Object z}) w=r;print(w is ({int a,String z}));}",
        "(T,{T value}) pair<T>(T x)=>(x,value:x);void main(){var r=pair(2);int x=r.value;print(x+r.$1);}",
        "T first<T>((T,String) r)=>r.$1;void main(){int x=first((1,'s'));print(x);}",
        "void main(){(List<int>,int Function(int)) r=([],(x)=>x+1);print(r.$2(2));}",
        "void main(){var rs=[(1,value:2),('s',value:true)];print(rs is List<(Object,{Object value})>);}",
        "void main(){(int,String)? r=(1,'s');if(r!=null){print(r.$1);}}",
        "void main(){print(($1:3).$1);print((1,$01:2).$01);print((1,$0:2).$0);}",
        "void main(){var (x,:name)=(1,name:'s');x=2;print(x);print(name);var (_,_)=(1,2);}",
        "void main(){print(((1,true),[1,2]));}",
    ] {
        check(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

/// Um padrão reserva todos os bindings antes do inicializador e mantém final imutável.
#[test]
fn destructuring_rejects_tdz_duplicates_and_mismatched_shapes() {
    for source in [
        "void main(){var x=1;{var (x,y)=(x,2);print(y);}}",
        "void main(){print(x);var (x,y)=(1,2);}",
        "void main(){var (x,x)=(1,2);}",
        "void main(){var (x,y)=(1,2);var x=3;}",
        "void main(){final (x,y)=(1,2);x=3;}",
        "void main(){var (x,y)=(1,name:2);}",
        "void main(){var (:other)=(name:2);}",
        "void main(){(int,int)? r=null;var (x,y)=r;}",
        "void main(){var (x,y)=(1,2);x='s';}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}

/// Campos readonly, tipos distintos e constantes não implementadas têm diagnóstico.
#[test]
fn unsupported_or_incompatible_records_are_not_silently_erased() {
    for source in [
        "void main(){var r=(1,2);r.$1=3;}",
        "void main(){var r=(1,2);print(r.$3);}",
        "void main(){(int,String) r=(1,true);}",
        "void main(){({int a}) r=(b:1);}",
        "void main(){print((_secret:1));}",
        "void main(){print((hashCode:1));}",
        "void main(){print((1,$1:2));}",
        "void main(){const r=(1,2);}",
        "void main(){var xs=[(1,2),(a:1)];}",
        "class A{} void main(){print((A(),));}",
        "void main(){print(((int x)=>x,));}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
