//! Contrato de `@DataClass()`: expansão, coexistência, diagnósticos, texto e cache.
//!
//! A expansão é conferida contra o mesmo sistema de tipos e o mesmo backend da
//! aplicação: cada caso bem-sucedido percorre lex, parse, expand, análise semântica
//! e emissão JavaScript, de modo que um membro gerado mal tipado falha aqui.
use dartforge_macros::{MacroPhase, MacroSession, PLAN_VERSION, augmentation_library, expand};
use dartforge_syntax::{BinaryOp, ExprKind, Program, StatementKind, Type};

/// Constrói AST a partir da mesma sintaxe aceita pelo pipeline público.
fn parse(source: &str) -> Program<'_> {
    let tokens = dartforge_lexer::lex(source).unwrap();
    dartforge_parser::parse(&tokens, source.len()).unwrap()
}

/// Reproduz o pipeline de unidade isolada do compilador, sem otimizações opcionais.
///
/// Devolve o JavaScript emitido; um erro indica que o código gerado pela macro não
/// sobrevive à análise semântica, não apenas que o teste está mal escrito.
fn compile(source: &str) -> Result<String, dartforge_diagnostics::Diagnostic> {
    let tokens = dartforge_lexer::lex(source)?;
    let mut ast = dartforge_parser::parse(&tokens, source.len())?;
    let expansion = expand(&mut ast, source.len())?;
    dartforge_hir::expand_mixins(&mut ast).map_err(|e| expansion.remap(e))?;
    let resolution = dartforge_semantic::analyze(&ast).map_err(|e| expansion.remap(e))?;
    Ok(dartforge_codegen::emit(&dartforge_hir::lower_resolved(
        ast, resolution,
    )))
}

/// Fonte usada em vários casos: um campo de cada forma escalar relevante.
const USUARIO: &str = "@DataClass() class Usuario{final String nome;final int idade;final String? apelido;final bool ativo;}";

/// Localiza um método pelo nome na classe indicada.
fn method<'a, 'b>(
    program: &'a Program<'b>,
    class: usize,
    name: &str,
) -> &'a dartforge_syntax::Function<'b> {
    program.classes[class]
        .methods
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("método {name} gerado"))
}

/// Os três membros surgem com assinaturas do subconjunto e o construtor é sintetizado.
#[test]
fn generates_constructor_copy_with_equals_and_describe() {
    let source = format!("{USUARIO}void main(){{}}");
    let mut program = parse(&source);
    let report = expand(&mut program, source.len()).unwrap();
    assert_eq!(report.applications, 1);
    // Construtor + copyWith + igualA + descrever.
    assert_eq!(report.generated_declarations, 4);
    let class = &program.classes[0];
    assert!(class.annotations.is_empty());
    let constructor = class.constructor.as_ref().unwrap();
    assert_eq!(constructor.parameters.len(), 4);
    assert!(
        constructor
            .parameters
            .iter()
            .zip(&class.fields)
            .all(|(p, f)| p.field == Some(f.name) && p.ty == f.ty)
    );
    // copyWith recebe todos os campos como posicionais anuláveis, na ordem declarada.
    let copy_with = method(&program, 0, "copyWith");
    assert_eq!(copy_with.return_type, Type::Class(class.id));
    assert_eq!(
        copy_with
            .parameters
            .iter()
            .map(|p| (p.name, p.ty))
            .collect::<Vec<_>>(),
        vec![
            ("nome", Type::NullableString),
            ("idade", Type::NullableInt),
            ("apelido", Type::NullableString),
            ("ativo", Type::NullableBool),
        ]
    );
    let StatementKind::Return(Some(value)) = &copy_with.body[0].kind else {
        panic!("return esperado")
    };
    let ExprKind::Construct {
        class_id,
        arguments,
    } = &value.kind
    else {
        panic!("construção esperada")
    };
    assert_eq!(*class_id, class.id);
    for (argument, field) in arguments.iter().zip(&class.fields) {
        let ExprKind::Binary { op, left, right } = &argument.kind else {
            panic!("?? esperado")
        };
        assert_eq!(*op, BinaryOp::IfNull);
        assert!(matches!(left.kind, ExprKind::Identifier(name) if name == field.name));
        let ExprKind::Member { receiver, name } = &right.kind else {
            panic!("this.campo esperado")
        };
        assert!(matches!(receiver.kind, ExprKind::This));
        assert_eq!(*name, field.name);
    }
    // igualA é um método comum; o subconjunto não declara operator ==.
    let equals = method(&program, 0, "igualA");
    assert_eq!(equals.return_type, Type::Bool);
    assert_eq!(equals.parameters.len(), 1);
    assert_eq!(equals.parameters[0].name, "outro");
    assert_eq!(equals.parameters[0].ty, Type::Class(class.id));
    assert_eq!(method(&program, 0, "descrever").return_type, Type::String);
    assert!(method(&program, 0, "descrever").parameters.is_empty());
}

/// Classe sem campos gera membros degenerados que continuam válidos.
#[test]
fn empty_class_generates_degenerate_but_valid_members() {
    let source = "@DataClass() class Vazia{}void main(){print(Vazia().igualA(Vazia()));print(Vazia().descrever());}";
    let mut program = parse(source);
    expand(&mut program, source.len()).unwrap();
    assert!(method(&program, 0, "copyWith").parameters.is_empty());
    let StatementKind::Return(Some(value)) = &method(&program, 0, "igualA").body[0].kind else {
        panic!("return esperado")
    };
    assert!(matches!(value.kind, ExprKind::Bool(true)));
    let StatementKind::Return(Some(value)) = &method(&program, 0, "descrever").body[0].kind else {
        panic!("return esperado")
    };
    assert!(matches!(&value.kind, ExprKind::OwnedString(text) if text == "Vazia()"));
    compile(source).unwrap();
}

/// descrever concatena literais e campos String; int e bool viram marcadores de tipo.
#[test]
fn describe_only_renders_string_fields_and_marks_the_others() {
    let source = format!("{USUARIO}void main(){{}}");
    let mut program = parse(&source);
    expand(&mut program, source.len()).unwrap();
    let StatementKind::Return(Some(value)) = &method(&program, 0, "descrever").body[0].kind else {
        panic!("return esperado")
    };
    // Achata a associação à esquerda de `+` para conferir os pedaços em ordem.
    fn flatten<'a, 'b>(expr: &'a dartforge_syntax::Expr<'b>, out: &mut Vec<&'a ExprKind<'b>>) {
        match &expr.kind {
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                flatten(left, out);
                flatten(right, out);
            }
            other => out.push(other),
        }
    }
    let mut pieces = Vec::new();
    flatten(value, &mut pieces);
    let literal = |kind: &ExprKind<'_>| match kind {
        ExprKind::OwnedString(text) => Some(text.clone()),
        _ => None,
    };
    assert_eq!(pieces.len(), 5);
    assert_eq!(literal(pieces[0]).unwrap(), "Usuario(nome: ");
    assert!(matches!(pieces[1], ExprKind::Member { name: "nome", .. }));
    assert_eq!(literal(pieces[2]).unwrap(), ", idade: <int>, apelido: ");
    // O campo anulável vira `this.apelido ?? 'null'` para permanecer String.
    let ExprKind::Binary {
        op: BinaryOp::IfNull,
        left,
        right,
    } = pieces[3]
    else {
        panic!("?? esperado no campo anulável")
    };
    assert!(matches!(
        left.kind,
        ExprKind::Member {
            name: "apelido",
            ..
        }
    ));
    assert!(matches!(&right.kind, ExprKind::OwnedString(text) if text == "null"));
    assert_eq!(literal(pieces[4]).unwrap(), ", ativo: <bool>)");
}

/// Todo o código gerado sobrevive à análise semântica e chega ao backend JavaScript.
#[test]
fn generated_members_pass_semantic_analysis_and_reach_javascript() {
    let source = format!(
        "{USUARIO}void main(){{var u=Usuario('a',1,null,true);var c=u.copyWith(null,2,'x',null);print(c.nome);print(c.idade);print(u.igualA(c));print(u.igualA(u));print(u.descrever());print(c.descrever());}}"
    );
    let javascript = compile(&source).unwrap();
    for member in ["copyWith", "igualA", "descrever"] {
        assert!(
            javascript.contains(member),
            "{member} ausente no JavaScript"
        );
    }
}

/// Um construtor já escrito é reaproveitado e nenhum outro é sintetizado.
#[test]
fn existing_constructor_is_reused_by_the_generated_members() {
    let source = "@DataClass() class Par{final int a;final int b;Par(this.a,this.b);}void main(){print(Par(1,2).copyWith(null,3).igualA(Par(1,3)));}";
    let mut program = parse(source);
    let report = expand(&mut program, source.len()).unwrap();
    assert_eq!(report.generated_declarations, 3);
    // O construtor conserva o intervalo escrito na fonte; nenhum span sintético o cobre.
    let constructor = program.classes[0].constructor.as_ref().unwrap();
    assert!(constructor.span.end <= source.len());
    assert!(
        report
            .origins
            .iter()
            .all(|origin| origin.generated != constructor.span)
    );
    compile(source).unwrap();
}

/// As duas macros coexistem gerando cinco membros disjuntos e um único construtor.
#[test]
fn coexists_with_json_codable_generating_disjoint_members() {
    let source = "@JsonCodable() @DataClass() class Item{final String nome;final int peso;}void main(){var i=Item.fromJson({'nome':'x','peso':2});print(i.toJson().length);print(i.copyWith(null,3).peso);print(i.igualA(i));print(i.descrever());}";
    let mut program = parse(source);
    let report = expand(&mut program, source.len()).unwrap();
    // Duas aplicações sobre a mesma classe; o construtor é contado uma única vez.
    assert_eq!(report.applications, 2);
    assert_eq!(report.phases[0].phase, MacroPhase::Types);
    assert_eq!(report.phases[0].applications, 0);
    assert_eq!(report.phases[1].phase, MacroPhase::Declarations);
    assert_eq!(report.phases[1].applications, 2);
    assert_eq!(report.phases[1].generated_declarations, 6);
    assert_eq!(report.phases[2].phase, MacroPhase::Definitions);
    assert_eq!(report.phases[2].applications, 2);
    assert_eq!(report.generated_declarations, 6);
    let class = &program.classes[0];
    assert_eq!(class.constructor.as_ref().unwrap().parameters.len(), 2);
    assert_eq!(class.factories.len(), 1);
    let mut names: Vec<_> = class.methods.iter().map(|m| m.name).collect();
    names.sort_unstable();
    assert_eq!(names, ["copyWith", "descrever", "igualA", "toJson"]);
    assert!(class.annotations.is_empty());
    // A ordem das anotações não muda nada do que é gerado.
    let swapped = source.replace("@JsonCodable() @DataClass()", "@DataClass() @JsonCodable()");
    let mut other = parse(&swapped);
    expand(&mut other, swapped.len()).unwrap();
    assert_eq!(format!("{:?}", other.classes[0]), format!("{class:?}"));
    compile(source).unwrap();
}

/// Cada diagnóstico aponta a anotação responsável e preserva a AST original.
#[test]
fn ineligible_targets_and_collisions_report_on_their_own_annotation() {
    for (source, needle, marker) in [
        (
            "@DataClass() @DataClass() class C{}void main(){}",
            "Duplicate DataClass application",
            "@DataClass()",
        ),
        (
            "class Base{} @DataClass() class C extends Base{final int x;}void main(){}",
            "DataClass currently requires a concrete class",
            "@DataClass()",
        ),
        (
            "mixin M{} @DataClass() class C with M{final int x;}void main(){}",
            "DataClass currently requires a concrete class",
            "@DataClass()",
        ),
        (
            "abstract class I{} @DataClass() class C implements I{final int x;}void main(){}",
            "DataClass currently requires a concrete class",
            "@DataClass()",
        ),
        (
            "@DataClass() abstract class C{final int x;}void main(){}",
            "DataClass currently requires a concrete class",
            "@DataClass()",
        ),
        (
            "@DataClass() mixin M{int x=1;}void main(){}",
            "DataClass currently requires a concrete class",
            "@DataClass()",
        ),
        (
            "@DataClass() mixin class M{final int x;}void main(){}",
            "DataClass currently requires a concrete class",
            "@DataClass()",
        ),
        (
            "@DataClass() enum E{a,b}void main(){}",
            "DataClass currently requires a concrete class",
            "@DataClass()",
        ),
        (
            "@DataClass() class C{final List<int> xs;}void main(){}",
            "DataClass requires scalar fields",
            "@DataClass()",
        ),
        (
            "@DataClass() class C{int x=1;}void main(){}",
            "DataClass requires scalar fields",
            "@DataClass()",
        ),
        (
            "@DataClass() class C{final int x;int copyWith()=>1;}void main(){}",
            "DataClass conflicts with existing copyWith/igualA/descrever member",
            "@DataClass()",
        ),
        (
            "@DataClass() class C{final int igualA;}void main(){}",
            "DataClass conflicts with existing copyWith/igualA/descrever member",
            "@DataClass()",
        ),
        (
            "@DataClass() class C{final int x;String descrever()=>'';}void main(){}",
            "DataClass conflicts with existing copyWith/igualA/descrever member",
            "@DataClass()",
        ),
        (
            "@DataClass() class C{final int x;C(this.x){print(x);}}void main(){}",
            "DataClass requires an empty constructor",
            "@DataClass()",
        ),
        (
            "@DataClass() class C{final int x;final int y;C(this.x);}void main(){}",
            "DataClass requires an empty constructor",
            "@DataClass()",
        ),
        // A colisão é conferida contra a anotação que reservou o nome, não a primeira.
        (
            "@JsonCodable() @DataClass() class C{final int x;int copyWith()=>1;}void main(){}",
            "DataClass conflicts with existing copyWith/igualA/descrever member",
            "@DataClass()",
        ),
        (
            "@DataClass() @JsonCodable() class C{final int x;int toJson()=>1;}void main(){}",
            "JsonCodable conflicts with existing toJson/fromJson member",
            "@JsonCodable()",
        ),
    ] {
        let mut program = parse(source);
        let original = format!("{program:?}");
        let error = expand(&mut program, source.len()).unwrap_err();
        assert!(
            error.message.contains(needle),
            "{source}: {}",
            error.message
        );
        let start = source.find(marker).unwrap();
        assert_eq!(
            &source[error.span.start..error.span.end.min(source.len())][..marker.len()],
            marker,
            "{source}"
        );
        assert_eq!(error.span.start, start, "{source}");
        // Falha em qualquer ponto deixa a AST intacta, inclusive as anotações.
        assert_eq!(format!("{program:?}"), original, "{source}");
    }
}

/// Uma falha em DataClass não expande a classe JsonCodable anterior.
#[test]
fn a_later_data_class_failure_preserves_the_whole_program() {
    let source =
        "@JsonCodable() class Boa{final int x;} @DataClass() class Ma{int x=1;}void main(){}";
    let mut program = parse(source);
    let original = format!("{program:?}");
    assert!(expand(&mut program, source.len()).is_err());
    assert_eq!(format!("{program:?}"), original);
    assert!(program.classes[0].constructor.is_none());
    assert!(program.classes[0].factories.is_empty());
}

/// Diagnósticos em nós gerados voltam à anotação e nomeiam a macro responsável.
#[test]
fn provenance_names_the_macro_that_generated_the_node() {
    let source = "@JsonCodable() @DataClass() class C{final int x;}void main(){}";
    let mut program = parse(source);
    let report = expand(&mut program, source.len()).unwrap();
    let mut prefixes = std::collections::BTreeSet::new();
    let mut end = source.len();
    for origin in &report.origins {
        // Intervalos gerados são exclusivos, crescentes e fora dos bytes originais.
        assert!(origin.generated.start > end);
        assert!(origin.generated.end > origin.generated.start);
        end = origin.generated.end;
        let mapped = report.remap(dartforge_diagnostics::Diagnostic::new(
            "erro gerado",
            origin.generated,
        ));
        assert_eq!(mapped.span, origin.annotation);
        prefixes.insert(mapped.message.split(':').next().unwrap().to_owned());
        assert_eq!(
            &source[origin.annotation.start..origin.annotation.end],
            match origin.kind {
                dartforge_macros::MacroKind::JsonCodable => "@JsonCodable()",
                dartforge_macros::MacroKind::DataClass => "@DataClass()",
            }
        );
    }
    assert_eq!(
        prefixes,
        ["DataClass".to_owned(), "JsonCodable".to_owned()].into()
    );
}

/// Duas expansões da mesma fonte produzem AST e relatório idênticos.
#[test]
fn expansion_is_deterministic() {
    let source = format!("{USUARIO}void main(){{}}");
    let mut first = parse(&source);
    let mut second = parse(&source);
    let a = expand(&mut first, source.len()).unwrap();
    let b = expand(&mut second, source.len()).unwrap();
    assert_eq!(format!("{first:?}"), format!("{second:?}"));
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}

/// O texto de aumento é estável byte a byte e ordenado por classe e por membro.
#[test]
fn augmentation_text_is_deterministic_and_ordered() {
    let source = "@DataClass() class Zeta{final int z;} @JsonCodable() @DataClass() class Alfa{final String nome;final int idade;final String? apelido;} @JsonCodable() class Meio{final bool ok;Meio(this.ok);}void main(){}";
    let program = parse(source);
    let text = augmentation_library(&program).unwrap();
    assert_eq!(text, augmentation_library(&parse(source)).unwrap());
    assert_eq!(
        text,
        "// Texto de inspeção gerado por dartforge-macros; não é reconsumido pelo compilador.\n\
\n\
augment class Alfa {\n\
\x20 augment Alfa(this.nome, this.idade, this.apelido);\n\
\x20 augment factory Alfa.fromJson(Map<String, Object?> json) {\n\
\x20   return Alfa(json['nome'] as String, json['idade'] as int, json['apelido'] as String?);\n\
\x20 }\n\
\x20 augment Alfa copyWith(String? nome, int? idade, String? apelido) {\n\
\x20   return Alfa(nome ?? this.nome, idade ?? this.idade, apelido ?? this.apelido);\n\
\x20 }\n\
\x20 augment String descrever() {\n\
\x20   return 'Alfa(nome: ' + this.nome + ', idade: <int>, apelido: ' + (this.apelido ?? 'null') + ')';\n\
\x20 }\n\
\x20 augment bool igualA(Alfa outro) {\n\
\x20   return this.nome == outro.nome && this.idade == outro.idade && this.apelido == outro.apelido;\n\
\x20 }\n\
\x20 augment Map<String, Object?> toJson() {\n\
\x20   return <String, Object?>{'nome': this.nome, 'idade': this.idade, 'apelido': this.apelido};\n\
\x20 }\n\
}\n\
\n\
augment class Meio {\n\
\x20 augment factory Meio.fromJson(Map<String, Object?> json) {\n\
\x20   return Meio(json['ok'] as bool);\n\
\x20 }\n\
\x20 augment Map<String, Object?> toJson() {\n\
\x20   return <String, Object?>{'ok': this.ok};\n\
\x20 }\n\
}\n\
\n\
augment class Zeta {\n\
\x20 augment Zeta(this.z);\n\
\x20 augment Zeta copyWith(int? z) {\n\
\x20   return Zeta(z ?? this.z);\n\
\x20 }\n\
\x20 augment String descrever() {\n\
\x20   return 'Zeta(z: <int>)';\n\
\x20 }\n\
\x20 augment bool igualA(Zeta outro) {\n\
\x20   return this.z == outro.z;\n\
\x20 }\n\
}\n"
    );
}

/// O texto descreve exatamente os membros que a expansão injeta na AST.
#[test]
fn augmentation_text_matches_the_expanded_ast() {
    let source =
        "@JsonCodable() @DataClass() class Item{final String nome;final int peso;}void main(){}";
    let before = parse(source);
    let text = augmentation_library(&before).unwrap();
    let members = text
        .lines()
        .filter(|line| line.starts_with("  augment "))
        .count();
    let mut program = parse(source);
    let report = expand(&mut program, source.len()).unwrap();
    assert_eq!(members, report.generated_declarations);
    let class = &program.classes[0];
    for name in class.methods.iter().chain(&class.factories).map(|m| m.name) {
        assert!(text.contains(name), "{name} ausente no texto de aumento");
    }
}

/// Programas sem macro devolvem apenas o cabeçalho; chamar após expandir devolve o mesmo.
#[test]
fn augmentation_text_is_empty_without_annotations() {
    let header =
        "// Texto de inspeção gerado por dartforge-macros; não é reconsumido pelo compilador.\n";
    assert_eq!(
        augmentation_library(&parse("class C{int x=1;}void main(){}")).unwrap(),
        header
    );
    let source = format!("{USUARIO}void main(){{}}");
    let mut program = parse(&source);
    expand(&mut program, source.len()).unwrap();
    assert_eq!(augmentation_library(&program).unwrap(), header);
}

/// O texto propaga o mesmo diagnóstico que a expansão produziria.
#[test]
fn augmentation_text_reports_the_same_diagnostics() {
    let source = "@DataClass() class C{final int x;int copyWith()=>1;}void main(){}";
    let program = parse(source);
    let text_error = augmentation_library(&program).unwrap_err();
    let mut copy = parse(source);
    let expansion_error = expand(&mut copy, source.len()).unwrap_err();
    assert_eq!(text_error.message, expansion_error.message);
    assert_eq!(text_error.span, expansion_error.span);
}

/// Planos de macros diferentes nunca se confundem no cache LRU da sessão.
#[test]
fn plan_cache_distinguishes_the_two_macros() {
    assert_eq!(PLAN_VERSION, 2);
    let mut session = MacroSession::default();
    // Mesmos campos, macros diferentes: três esquemas distintos, três faltas.
    for annotations in [
        "@JsonCodable()",
        "@DataClass()",
        "@JsonCodable() @DataClass()",
    ] {
        let source = format!("{annotations} class C{{final int a;final String b;}}void main(){{}}");
        let mut program = parse(&source);
        let report = session.expand(&mut program, source.len()).unwrap();
        assert_eq!(report.plan_misses, 1, "{annotations}");
        assert_eq!(report.plan_hits, 0, "{annotations}");
    }
    assert_eq!(session.stats().misses, 3);
    assert_eq!(session.stats().entries, 3);
    // Repetir as mesmas formas agora acerta, sem nenhuma falta adicional.
    for annotations in [
        "@JsonCodable()",
        "@DataClass()",
        "@JsonCodable() @DataClass()",
    ] {
        let source =
            format!("{annotations} class Outra{{final int a;final String b;}}void main(){{}}");
        let mut program = parse(&source);
        let report = session.expand(&mut program, source.len()).unwrap();
        assert_eq!(report.plan_hits, 1, "{annotations}");
        assert_eq!(report.plan_misses, 0, "{annotations}");
    }
    assert_eq!(session.stats().hits, 3);
    assert_eq!(session.stats().misses, 3);
}

/// Acerto de plano não reutiliza nós, spans nem proveniência entre expansões.
#[test]
fn plan_cache_hit_still_materializes_fresh_nodes() {
    let source = "@DataClass() class C{final int a;}void main(){}";
    let mut session = MacroSession::default();
    let mut first = parse(source);
    let a = session.expand(&mut first, source.len()).unwrap();
    assert_eq!(a.plan_misses, 1);
    let mut second = parse(source);
    let b = session.expand(&mut second, source.len()).unwrap();
    assert_eq!(b.plan_hits, 1);
    assert_eq!(b.plan_misses, 0);
    assert_eq!(a.materialized_nodes, b.materialized_nodes);
    assert!(a.materialized_nodes > 0);
    assert_eq!(format!("{first:?}"), format!("{second:?}"));
    // Uma sessão sem retenção erra sempre e produz exatamente a mesma AST.
    let mut disabled = MacroSession::with_limits(0, 0);
    let mut third = parse(source);
    let c = disabled.expand(&mut third, source.len()).unwrap();
    assert_eq!(c.plan_misses, 1);
    assert_eq!(c.plan_hits, 0);
    assert_eq!(format!("{third:?}"), format!("{first:?}"));
}

/// Expulsão por limite de entradas volta a errar sem alterar o resultado gerado.
#[test]
fn plan_cache_eviction_is_observable_and_harmless() {
    let mut session = MacroSession::with_limits(1, usize::MAX);
    let sources = [
        "@DataClass() class A{final int a;}void main(){}",
        "@DataClass() class B{final String b;}void main(){}",
    ];
    let mut reports = Vec::new();
    for source in sources.iter().chain(sources.iter()) {
        let mut program = parse(source);
        reports.push(session.expand(&mut program, source.len()).unwrap());
    }
    assert!(reports.iter().all(|r| r.plan_hits == 0));
    assert!(reports.iter().all(|r| r.plan_misses == 1));
    assert_eq!(session.stats().evictions, 3);
    assert_eq!(session.stats().entries, 1);
}

/// Executa o JavaScript emitido, mantendo stderr para diagnosticar diferenças.
fn run(javascript: &str) -> std::process::Output {
    std::process::Command::new("node")
        .args(["--input-type=module", "--eval", javascript])
        .output()
        .unwrap()
}

/// O comportamento observável dos três membros confere com o contrato documentado.
#[test]
#[ignore = "requer Node.js no PATH"]
fn generated_members_behave_as_documented_at_runtime() {
    let source = format!(
        "{USUARIO}void main(){{\
var u=Usuario('Dart',15,null,true);\
var c=u.copyWith(null,16,'apelido',null);\
print(c.nome);print(c.idade);print(c.apelido);print(c.ativo);\
print(u.igualA(u));print(u.igualA(c));print(u.igualA(Usuario('Dart',15,null,true)));\
print(u.descrever());print(c.descrever());\
print(c.copyWith(null,null,null,null).descrever());\
}}"
    );
    let output = run(&compile(&source).unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        // copyWith mantém o campo quando o argumento é null, inclusive em campos
        // anuláveis: a terceira linha do último descrever conserva 'apelido'.
        "Dart\n16\napelido\ntrue\n\
true\nfalse\ntrue\n\
Usuario(nome: Dart, idade: <int>, apelido: null, ativo: <bool>)\n\
Usuario(nome: Dart, idade: <int>, apelido: apelido, ativo: <bool>)\n\
Usuario(nome: Dart, idade: <int>, apelido: apelido, ativo: <bool>)\n"
    );
}
