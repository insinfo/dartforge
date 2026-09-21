//! Emissão textual das augmentations equivalentes ao que a expansão injeta.
//!
//! # Este texto não volta ao compilador
//!
//! O que esta API produz é **material de inspeção, depuração e prova de
//! proveniência**. Nada aqui é reconsumido pelo pipeline: a expansão real
//! acontece em AST, em [`crate::expand`], e nunca passa por reparsing. Escrever
//! o resultado em disco e compilá-lo não é um caminho suportado — a sintaxe
//! `augment` sequer é aceita pelo parser do subconjunto.
//!
//! O objetivo é permitir ler, revisar e versionar em texto exatamente os membros
//! que a macro acrescentou, na forma de biblioteca de aumento:
//!
//! ```text
//! augment class Usuario {
//!   augment Usuario(this.nome, this.idade);
//!   augment factory Usuario.fromJson(Map<String, Object?> json) {
//!     return Usuario(json['nome'] as String, json['idade'] as int);
//!   }
//! }
//! ```
//!
//! # Determinismo
//!
//! A saída é byte a byte estável para a mesma entrada. As classes saem ordenadas
//! por nome e, em empate, por identidade da declaração; os membros de cada classe
//! saem em ordem canônica (construtor, fábricas por nome, métodos por nome).
//! Nenhuma iteração depende de tabela hash, de spans ou da ordem do arquivo.
use crate::{MacroPlan, data_class, type_text, validate_class};
use dartforge_diagnostics::Diagnostic;
use dartforge_syntax::Program;

/// Cabeçalho fixo que marca o texto como material de inspeção.
const HEADER: &str =
    "// Texto de inspeção gerado por dartforge-macros; não é reconsumido pelo compilador.\n";

/// Membro sintetizado, com o texto Dart correspondente ao que a macro injetou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AugmentedMember {
    /// Nome do membro: o nome da classe para o construtor, o nome próprio nos demais.
    pub name: String,
    /// Declaração completa, iniciada por `augment` e sem indentação externa.
    pub text: String,
}
/// Aumentos de uma classe, prontos para serem escritos em ordem estável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassAugmentation {
    /// Identidade da declaração, usada para desempatar nomes iguais.
    pub id: u32,
    pub class: String,
    /// Membros em ordem canônica: construtor, fábricas por nome, métodos por nome.
    pub members: Vec<AugmentedMember>,
}

/// Escapa um literal Dart de aspas simples, inclusive o `$` de interpolação.
///
/// Identificadores Dart podem conter `$`, então um nome de campo copiado para
/// dentro de `descrever()` precisa ser escapado para não virar interpolação.
fn escape(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => result.push_str("\\\\"),
            '\'' => result.push_str("\\'"),
            '$' => result.push_str("\\$"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            other => result.push(other),
        }
    }
    result
}

/// Escreve um corpo de método de uma única instrução `return`.
fn body(signature: &str, value: &str) -> String {
    format!("augment {signature} {{\n  return {value};\n}}")
}

/// Reproduz o construtor posicional com initializing formals.
fn constructor(class: &str, plan: &MacroPlan) -> AugmentedMember {
    let parameters = plan
        .fields
        .iter()
        .map(|field| format!("this.{}", field.name))
        .collect::<Vec<_>>()
        .join(", ");
    AugmentedMember {
        name: class.to_owned(),
        text: format!("augment {class}({parameters});"),
    }
}

/// Reproduz `fromJson` e `toJson` com a forma `Map<String, Object?>` realmente gerada.
///
/// A macro não usa `Map<String, dynamic>`: `dynamic` não existe no subconjunto e o
/// tipo reificado é `Map<String, Object?>`, com cast explícito por campo.
fn json_members(class: &str, plan: &MacroPlan) -> [AugmentedMember; 2] {
    let arguments = plan
        .fields
        .iter()
        .map(|field| format!("json['{}'] as {}", escape(&field.name), type_text(field.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let entries = plan
        .fields
        .iter()
        .map(|field| format!("'{}': this.{}", escape(&field.name), field.name))
        .collect::<Vec<_>>()
        .join(", ");
    [
        AugmentedMember {
            name: String::from("fromJson"),
            text: body(
                &format!("factory {class}.fromJson(Map<String, Object?> json)"),
                &format!("{class}({arguments})"),
            ),
        },
        AugmentedMember {
            name: String::from("toJson"),
            text: body(
                "Map<String, Object?> toJson()",
                &format!("<String, Object?>{{{entries}}}"),
            ),
        },
    ]
}

/// Reproduz `copyWith`, `igualA` e `descrever` com os limites já documentados.
fn data_members(class: &str, plan: &MacroPlan) -> [AugmentedMember; 3] {
    let parameters = plan
        .fields
        .iter()
        .map(|field| format!("{} {}", type_text(crate::nullable(field.ty)), field.name))
        .collect::<Vec<_>>()
        .join(", ");
    let arguments = plan
        .fields
        .iter()
        .map(|field| format!("{} ?? this.{}", field.name, field.name))
        .collect::<Vec<_>>()
        .join(", ");
    let comparisons = plan
        .fields
        .iter()
        .map(|field| {
            format!(
                "this.{} == {}.{}",
                field.name,
                data_class::OTHER,
                field.name
            )
        })
        .collect::<Vec<_>>()
        .join(" && ");
    let described = data_class::describe_pieces(class, plan)
        .iter()
        .map(|piece| match piece {
            data_class::DescribePiece::Literal(text) => format!("'{}'", escape(text)),
            data_class::DescribePiece::Field {
                index,
                nullable: true,
            } => format!("(this.{} ?? 'null')", plan.fields[*index].name),
            data_class::DescribePiece::Field {
                index,
                nullable: false,
            } => format!("this.{}", plan.fields[*index].name),
        })
        .collect::<Vec<_>>()
        .join(" + ");
    [
        AugmentedMember {
            name: String::from(data_class::COPY_WITH),
            text: body(
                &format!("{class} {}({parameters})", data_class::COPY_WITH),
                &format!("{class}({arguments})"),
            ),
        },
        AugmentedMember {
            name: String::from(data_class::EQUALS),
            text: body(
                &format!("bool {}({class} {})", data_class::EQUALS, data_class::OTHER),
                if comparisons.is_empty() {
                    "true"
                } else {
                    &comparisons
                },
            ),
        },
        AugmentedMember {
            name: String::from(data_class::DESCRIBE),
            text: body(&format!("String {}()", data_class::DESCRIBE), &described),
        },
    ]
}

/// Reúne os aumentos de uma classe já validada em ordem canônica de membro.
fn render(id: u32, class: &str, plan: &MacroPlan) -> ClassAugmentation {
    let mut members = Vec::new();
    if plan.generate_constructor {
        members.push(constructor(class, plan));
    }
    // Fábricas antes de métodos; dentro de cada grupo, a ordem é alfabética.
    let mut factories = Vec::new();
    let mut methods = Vec::new();
    if plan.json_codable {
        let [from_json, to_json] = json_members(class, plan);
        factories.push(from_json);
        methods.push(to_json);
    }
    if plan.data_class {
        methods.extend(data_members(class, plan));
    }
    factories.sort_by(|a, b| a.name.cmp(&b.name));
    methods.sort_by(|a, b| a.name.cmp(&b.name));
    members.append(&mut factories);
    members.append(&mut methods);
    ClassAugmentation {
        id,
        class: class.to_owned(),
        members,
    }
}

/// Descreve, por classe, os membros que a expansão acrescentaria ao programa.
///
/// Recebe o programa **antes** de [`crate::expand`], porque a expansão consome as
/// anotações; chamar depois devolve uma lista vazia. A validação é a mesma da
/// expansão, então os diagnósticos coincidem byte a byte com os dela.
///
/// ```
/// let source = "@DataClass() class Ponto{final int x;final String rotulo;}void main(){}";
/// let tokens = dartforge_lexer::lex(source).unwrap();
/// let program = dartforge_parser::parse(&tokens, source.len()).unwrap();
/// let classes = dartforge_macros::augmentations(&program).unwrap();
/// assert_eq!(classes[0].class, "Ponto");
/// let names: Vec<_> = classes[0].members.iter().map(|m| m.name.as_str()).collect();
/// assert_eq!(names, ["Ponto", "copyWith", "descrever", "igualA"]);
/// ```
/// # Erros
/// Retorna os mesmos diagnósticos de elegibilidade e colisão da expansão.
pub fn augmentations(program: &Program<'_>) -> Result<Vec<ClassAugmentation>, Diagnostic> {
    let mut result = Vec::new();
    for class in &program.classes {
        if let Some(targets) = validate_class(class)? {
            let plan = crate::plan_from(class, targets);
            result.push(render(class.id, class.name, &plan));
        }
    }
    result.sort_by(|a, b| a.class.cmp(&b.class).then(a.id.cmp(&b.id)));
    Ok(result)
}

/// Emite a biblioteca de aumento inteira como texto Dart determinístico.
///
/// A saída existe para inspeção e prova de proveniência; **não** é um arquivo que
/// o compilador leia de volta, como explica a documentação do módulo. Programas
/// sem macro devolvem apenas o cabeçalho de comentário.
///
/// ```
/// let source = "@JsonCodable() class Ponto{final int x;}void main(){}";
/// let tokens = dartforge_lexer::lex(source).unwrap();
/// let program = dartforge_parser::parse(&tokens, source.len()).unwrap();
/// let text = dartforge_macros::augmentation_library(&program).unwrap();
/// assert!(text.contains("augment class Ponto {"));
/// assert!(text.contains("  augment Ponto(this.x);"));
/// assert_eq!(text, dartforge_macros::augmentation_library(&program).unwrap());
/// ```
/// # Erros
/// Retorna os mesmos diagnósticos de elegibilidade e colisão da expansão.
pub fn augmentation_library(program: &Program<'_>) -> Result<String, Diagnostic> {
    let mut text = String::from(HEADER);
    for augmentation in augmentations(program)? {
        text.push_str(&format!("\naugment class {} {{\n", augmentation.class));
        for member in &augmentation.members {
            for line in member.text.lines() {
                text.push_str("  ");
                text.push_str(line);
                text.push('\n');
            }
        }
        text.push_str("}\n");
    }
    Ok(text)
}

/// Mantém `type_text` honesto quanto aos seis escalares aceitos pela validação.
#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_syntax::Type;

    /// Os seis tipos aceitos têm grafia Dart própria e `nullable` é idempotente.
    #[test]
    fn scalar_types_render_and_nullable_is_idempotent() {
        for (ty, text, nullable_text) in [
            (Type::Int, "int", "int?"),
            (Type::Bool, "bool", "bool?"),
            (Type::String, "String", "String?"),
            (Type::NullableInt, "int?", "int?"),
            (Type::NullableBool, "bool?", "bool?"),
            (Type::NullableString, "String?", "String?"),
        ] {
            assert_eq!(type_text(ty), text);
            assert_eq!(type_text(crate::nullable(ty)), nullable_text);
        }
    }

    /// Nomes com `$` não podem virar interpolação dentro do literal emitido.
    #[test]
    fn dollar_and_quotes_are_escaped() {
        assert_eq!(escape("a$b"), "a\\$b");
        assert_eq!(escape("a'b\\c"), "a\\'b\\\\c");
    }
}
