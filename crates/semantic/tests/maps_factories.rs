//! Mapas tipados e factories nomeadas têm contratos independentes de instância.
/// Verifica fontes completas, inclusive declarações nunca chamadas.
fn check(source: &str) -> Result<dartforge_syntax::Resolution, String> {
    let tokens = dartforge_lexer::lex(source).map_err(|e| e.message)?;
    let program = dartforge_parser::parse(&tokens, source.len()).map_err(|e| e.message)?;
    dartforge_semantic::analyze(&program).map_err(|e| e.message)
}
/// Mapas propagam contexto, nulabilidade de lookup e substituição genérica.
#[test]
fn map_types_and_factories_validate() {
    for source in [
        "void main(){Map<String,int> m={};m['a']=1;int? n=m['a'];print(n);print(m.length);}",
        "void main(){var m={'x':1,'y':2};print(m);print(m['missing']==null);}",
        "void main(){var m={'x':1};print(m[1]);print(m[null]);}",
        "void main(){Map<String,Object?> m={'x':1,'y':'s','z':null};print(m['x'] as int);}",
        "T? find<T>(Map<String,T> m)=>m['x'];void main(){int? x=find({'x':1});print(x);}",
        "class C{final int x;C(this.x);factory C.from(Map<String,Object?> m)=>C(m['x'] as int);}void main(){print(C.from({'x':2}).x);}",
        "class C{int x=0;C();factory C.make()=>C();}void main(){C c=C.make();print(c.x);}",
        "void main(){Map<String,Object> wide=<String,int>{'x':1};wide['x']='s';}",
    ] {
        check(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    }
}
/// Factories não herdam nomes nem recebem this; lookup não promete valor existente.
#[test]
fn invalid_maps_and_factories_fail() {
    for source in [
        "void main(){Map<int,int> m=<int,int>{1:2};}",
        "void main(){Map<String,int> m={'x':'s'};}",
        "void main(){Map<String,int> m={};int x=m['absent'];}",
        "void main(){Map<String,int> m={};m['x']='s';}",
        "void main(){Map<String,int>? m=null;print(m['x']);}",
        "void main(){const m=<String,int>{'x':1};}",
        "class C{factory C.make()=>this;}void main(){}",
        "class C{factory C.make()=>C();}void main(){}",
        "C x()=>C();class C{int x()=>1;factory C.make()=>x();}void main(){}",
        "class C{factory C.make()=>1;}void main(){}",
        "class C{factory C.make(int x)=>C();}void main(){C.make();}",
        "class C{factory C.make()=>C();}class D extends C{}void main(){D.make();}",
        "class C{factory C.make()=>C();}void main(){int C=1;C.make();}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}
