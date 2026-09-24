//! Códigos e identificadores canônicos de diagnóstico alinhados ao `analyzer` do Dart SDK.
//!
//! Usados para categorização, mensagens de erro estruturadas e testes negativos
//! de conformidade com o compilador oficial.

/// Código de erro canônico do Dart Analyzer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagnosticCode {
    pub name: &'static str,
    pub template: &'static str,
}

impl DiagnosticCode {
    pub const fn new(name: &'static str, template: &'static str) -> Self {
        Self { name, template }
    }
}

// Erros de tipagem e atribuição
pub const ARGUMENT_TYPE_NOT_ASSIGNABLE: DiagnosticCode = DiagnosticCode::new(
    "argument_type_not_assignable",
    "O tipo de argumento '{0}' não pode ser atribuído ao parâmetro do tipo '{1}'.",
);

pub const RETURN_OF_INVALID_TYPE: DiagnosticCode = DiagnosticCode::new(
    "return_of_invalid_type",
    "Um valor do tipo '{0}' não pode ser retornado de uma função com retorno '{1}'.",
);

pub const INVALID_ASSIGNMENT: DiagnosticCode = DiagnosticCode::new(
    "invalid_assignment",
    "Um valor do tipo '{0}' não pode ser atribuído a uma variável do tipo '{1}'.",
);

pub const ASSIGNMENT_TO_FINAL_LOCAL: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_final_local",
    "A variável local final '{0}' só pode ser atribuída uma vez.",
);

pub const ASSIGNMENT_TO_FINAL: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_final",
    "A variável final não tem setter.",
);

pub const ASSIGNMENT_TO_FINAL_NO_SETTER: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_final_no_setter",
    "Não há setter para '{0}' na classe '{1}'.",
);

pub const ASSIGNMENT_TO_METHOD: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_method",
    "Não é possível atribuir um valor a um método.",
);

pub const UNDEFINED_EXTENSION_SETTER: DiagnosticCode = DiagnosticCode::new(
    "undefined_extension_setter",
    "Setter de extensão não encontrado",
);

pub const UNDEFINED_EXTENSION_GETTER: DiagnosticCode = DiagnosticCode::new(
    "undefined_extension_getter",
    "Getter de extensão não encontrado",
);

pub const UNDEFINED_EXTENSION_METHOD: DiagnosticCode = DiagnosticCode::new(
    "undefined_extension_method",
    "Método de extensão não encontrado",
);

pub const STATIC_ACCESS_TO_INSTANCE_MEMBER: DiagnosticCode = DiagnosticCode::new(
    "static_access_to_instance_member",
    "Acesso estático a membro de instância",
);

pub const INVOCATION_OF_EXTENSION_WITHOUT_CALL: DiagnosticCode = DiagnosticCode::new(
    "invocation_of_extension_without_call",
    "Invocação de extensão sem call",
);

pub const UNDEFINED_EXTENSION_OPERATOR: DiagnosticCode = DiagnosticCode::new(
    "undefined_extension_operator",
    "Operador de extensão não encontrado",
);

pub const EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER: DiagnosticCode = DiagnosticCode::new(
    "extension_override_access_to_static_member",
    "An extension override can't be used to access a static member from an extension.",
);

pub const ASSIGNMENT_TO_CONST: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_const",
    "Variáveis constantes não podem receber nova atribuição.",
);

pub const ASSIGNMENT_TO_TYPE: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_type",
    "Tipos não podem receber uma atribuição.",
);

pub const ASSIGNMENT_TO_FUNCTION: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_function",
    "Funções não podem receber uma atribuição.",
);

pub const NOT_INITIALIZED_NON_NULLABLE_VARIABLE: DiagnosticCode = DiagnosticCode::new(
    "not_initialized_non_nullable_variable",
    "A variável não-anulável '{0}' deve ser definitivamente atribuída antes do uso.",
);

pub const DEFINITELY_UNASSIGNED_VARIABLE: DiagnosticCode = DiagnosticCode::new(
    "definitely_unassigned_variable",
    "A variável '{0}' é lida antes de ser atribuída.",
);

// Erros de resolução de membros e identificadores
pub const UNDEFINED_IDENTIFIER: DiagnosticCode = DiagnosticCode::new(
    "undefined_identifier",
    "Nome indefinido '{0}'.",
);

pub const REFERENCED_BEFORE_DECLARATION: DiagnosticCode = DiagnosticCode::new(
    "referenced_before_declaration",
    "A variável local '{0}' não pode ser referenciada antes de sua declaração.",
);

pub const UNDEFINED_GETTER: DiagnosticCode = DiagnosticCode::new(
    "undefined_getter",
    "O getter '{0}' não está definido para o tipo '{1}'.",
);

pub const UNDEFINED_SETTER: DiagnosticCode = DiagnosticCode::new(
    "undefined_setter",
    "O setter '{0}' não está definido para o tipo '{1}'.",
);

pub const UNDEFINED_METHOD: DiagnosticCode = DiagnosticCode::new(
    "undefined_method",
    "O método '{0}' não está definido para o tipo '{1}'.",
);

pub const UNDEFINED_OPERATOR: DiagnosticCode = DiagnosticCode::new(
    "undefined_operator",
    "O operador '{0}' não está definido para o tipo '{1}'.",
);

pub const AMBIGUOUS_EXTENSION_MEMBER_ACCESS: DiagnosticCode = DiagnosticCode::new(
    "ambiguous_extension_member_access",
    "Membro ambíguo '{0}' encontrado nas extensões '{1}' e '{2}'.",
);

// Erros de fluxo e controle
pub const AWAIT_IN_WRONG_CONTEXT: DiagnosticCode = DiagnosticCode::new(
    "await_in_wrong_context",
    "A expressão 'await' só pode ser usada em funções assíncronas.",
);

pub const YIELD_EACH_IN_NON_GENERATOR: DiagnosticCode = DiagnosticCode::new(
    "yield_each_in_non_generator",
    "A instrução 'yield*' só pode ser usada em funções geradoras.",
);

pub const NON_BOOL_CONDITION: DiagnosticCode = DiagnosticCode::new(
    "non_bool_condition",
    "Condições devem ter o tipo estático 'bool'.",
);

pub const NON_BOOL_NEGATION_EXPRESSION: DiagnosticCode = DiagnosticCode::new(
    "non_bool_negation_expression",
    "A expressão negada deve ter o tipo estático 'bool'.",
);

// Erros de argumentos e parâmetros
pub const EXTRA_POSITIONAL_ARGUMENTS: DiagnosticCode = DiagnosticCode::new(
    "extra_positional_arguments",
    "Muitos argumentos posicionais: esperava {0}, mas recebeu {1}.",
);

pub const NOT_ENOUGH_POSITIONAL_ARGUMENTS: DiagnosticCode = DiagnosticCode::new(
    "not_enough_positional_arguments",
    "Poucos argumentos posicionais: esperava pelo menos {0}, mas recebeu {1}.",
);

pub const MISSING_REQUIRED_ARGUMENT: DiagnosticCode = DiagnosticCode::new(
    "missing_required_argument",
    "O parâmetro nomeado obrigatório '{0}' não foi fornecido.",
);

pub const UNDEFINED_NAMED_PARAMETER: DiagnosticCode = DiagnosticCode::new(
    "undefined_named_parameter",
    "O parâmetro nomeado '{0}' não está definido para a função.",
);

pub const TYPE_ARGUMENT_NOT_MATCHING_BOUNDS: DiagnosticCode = DiagnosticCode::new(
    "type_argument_not_matching_bounds",
    "O argumento de tipo '{0}' não estende o limite '{1}'.",
);

// Erros de constantes
pub const CONST_WITH_NON_CONSTANT_ARGUMENT: DiagnosticCode = DiagnosticCode::new(
    "const_with_non_constant_argument",
    "Argumentos de uma instanciação const devem ser expressões constantes.",
);

pub const CONST_EVAL_THROWS_EXCEPTION: DiagnosticCode = DiagnosticCode::new(
    "const_eval_throws_exception",
    "A avaliação da expressão constante lançou uma exceção: {0}.",
);

pub const EQUAL_ELEMENTS_IN_CONST_SET: DiagnosticCode = DiagnosticCode::new(
    "equal_elements_in_const_set",
    "Dois elementos em um conjunto constante avaliam para o mesmo valor.",
);

pub const EQUAL_KEYS_IN_CONST_MAP: DiagnosticCode = DiagnosticCode::new(
    "equal_keys_in_const_map",
    "Duas chaves em um mapa constante avaliam para o mesmo valor.",
);

pub const CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE: DiagnosticCode = DiagnosticCode::new(
    "const_initialized_with_non_constant_value",
    "A variável const deve ser inicializada com um valor constante.",
);

// Avisos de código morto e construções desnecessárias
pub const INVALID_NULL_AWARE_OPERATOR: DiagnosticCode = DiagnosticCode::new(
    "invalid_null_aware_operator",
    "O operador de verificação de nulo é desnecessário porque o receptor não pode ser nulo.",
);

pub const UNNECESSARY_CAST: DiagnosticCode = DiagnosticCode::new(
    "unnecessary_cast",
    "O cast de tipo é desnecessário porque o valor já possui este tipo.",
);

pub const UNNECESSARY_TYPE_CHECK_TRUE: DiagnosticCode = DiagnosticCode::new(
    "unnecessary_type_check_true",
    "O teste de tipo sempre avalia para true.",
);

pub const DEAD_CODE: DiagnosticCode = DiagnosticCode::new(
    "dead_code",
    "Código inalcançável (dead code).",
);

// Erros de linguagem dos recursos 3.7–3.13 (docs/VERSOES-LINGUAGEM.md §3).
//
// Os diagnósticos de `types` são avisos, porque a inferência ainda tem
// lacunas; estes não: são o que o CFE recusa nos recursos novos e o que um
// programa negativo do corpus precisa ver recusado. A mensagem começa com
// [`ERRO_DE_LINGUAGEM`], e quem compila (`compile-js`, `dartforge-jsprod`,
// nativo) aborta como aborta nos erros de carga.

/// Prefixo das mensagens de erro de linguagem.
pub const ERRO_DE_LINGUAGEM: &str = "erro de linguagem: ";

/// A mensagem é de um erro de linguagem (e não de um aviso de tipos)?
pub fn e_erro_de_linguagem(mensagem: &str) -> bool {
    mensagem.starts_with(ERRO_DE_LINGUAGEM)
}

pub const WILDCARD_NAO_LIGA: DiagnosticCode = DiagnosticCode::new(
    "undefined_identifier",
    "nenhum '_' visível: numa biblioteca 3.7+, local e parâmetro chamados '_' são curingas e não ligam nome",
);

pub const DOT_SHORTHAND_SEM_CONTEXTO: DiagnosticCode = DiagnosticCode::new(
    "dot_shorthand_missing_context",
    "o atalho de ponto '.{0}' precisa de um tipo de contexto que denote uma classe, mixin, enum ou extension type",
);

pub const DOT_SHORTHAND_SEM_MEMBRO: DiagnosticCode = DiagnosticCode::new(
    "dot_shorthand_undefined_member",
    "'{1}' não tem membro estático nem construtor '{0}' para o atalho de ponto",
);

pub const CONSTRUTOR_PRIMARIO: DiagnosticCode = DiagnosticCode::new(
    "primary_constructor",
    "construtor primário: {0}",
);
