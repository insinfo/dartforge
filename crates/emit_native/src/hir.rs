//! HIR própria do DartForge Nativo (docs/NATIVO.md e PLANO.md § Incremento 28).
//!
//! A HIR é uma representação em CFG (Control Flow Graph) SSA onde:
//! - Chamadas estão resolvidas (estática, seletor de interface, ou dinâmica).
//! - Casts implícitos foram tornados explícitos.
//! - Açúcares sintáticos foram desugarados.
//! - O backend LLVM (e futuramente Cranelift) consome diretamente esta estrutura.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

/// Tipos primitivos e gerenciados da HIR nativa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    /// Inteiro assinado de 64 bits com estouro modular (Dart `int`).
    I64,
    /// Byte de 8 bits (para booleanos no runtime Rust).
    I8,
    /// Ponto flutuante IEEE 754 de 64 bits (Dart `double`).
    F64,
    /// Booleano de 1 bit (Dart `bool`).
    I1,
    /// Handle de objeto gerenciado no heap (rastreado pelo GC, 0 = null).
    Ref,
    /// Retorno vazio.
    Void,
    /// Endereço de um `alloca` (local em memória, R6). Nunca é valor Dart.
    Ptr,
}

impl Type {
    /// Tipo LLVM IR textual correspondente.
    pub fn llvm_ir(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::I8 => "i8",
            Self::F64 => "double",
            Self::I1 => "i1",
            Self::Ref => "i64",
            Self::Void => "void",
            Self::Ptr => "ptr",
        }
    }
}

/// Constantes suportadas na HIR.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Double(f64),
    Bool(bool),
    String(String),
    Null,
}

/// Operando de instrução: uma constante direta ou um resultado SSA anterior.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Constant(Constant),
    Val(ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ICmpOp {
    Eq,
    Ne,
    Slt,
    Sle,
    Sgt,
    Sge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FCmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Instruções que produzem um valor (ou efetuam efeito de escrita).
#[derive(Debug, Clone)]
pub enum Instruction {
    /// Carrega uma constante em um identificador SSA.
    Const(Constant),

    // Aritmética e lógica inteira
    Add(Operand, Operand),
    Sub(Operand, Operand),
    Mul(Operand, Operand),
    SDiv(Operand, Operand), // ~/
    SRem(Operand, Operand), // %
    Shl(Operand, Operand),
    AShr(Operand, Operand),
    /// `>>>`: deslocamento lógico.
    LShr(Operand, Operand),
    And(Operand, Operand),
    Or(Operand, Operand),
    Xor(Operand, Operand),
    Neg(Operand),
    Not(Operand), // ~

    // Aritmética de ponto flutuante
    FAdd(Operand, Operand),
    FSub(Operand, Operand),
    FMul(Operand, Operand),
    FDiv(Operand, Operand), // /
    FNeg(Operand),

    // Comparações
    ICmp(ICmpOp, Operand, Operand),
    FCmp(FCmpOp, Operand, Operand),
    LNot(Operand), // ! (booleano)

    // Conversões primitivas
    IntToDouble(Operand),
    DoubleToInt(Operand),
    ZExt { op: Operand, from: Type, to: Type },
    Trunc { op: Operand, from: Type, to: Type },
    /// Reinterpreta os bits entre `i64` e `double` (valor lido do heap ou
    /// gravado nele). Não é conversão numérica: essa é `IntToDouble`.
    Bitcast { op: Operand, to: Type },
    /// Escalar (`I64`/`F64`/`I1`) numa posição `Ref`: caixa no heap (R3).
    Box { op: Operand, from: Type },
    /// Caixa de volta ao escalar; null ou outro tipo lança `TypeError`
    /// (exceção pendente — quem emite verifica, como numa chamada).
    Unbox { op: Operand, to: Type },

    // Alocações de heap
    AllocObject {
        class_id: u32,
        fields: Vec<Operand>,
    },
    AllocList {
        elements: Vec<(Operand, u8)>,
    },
    AllocMap {
        entries: Vec<((Operand, u8), (Operand, u8))>,
    },
    AllocSet {
        elements: Vec<(Operand, u8)>,
    },
    AllocRecord {
        elements: Vec<(Operand, u8)>,
    },
    AllocCell {
        value: Operand,
    },
    AllocEnv {
        values: Vec<Operand>,
    },
    AllocClosure {
        code_symbol: String,
        env: Operand,
    },

    // Memória local e ponteiros de pilha
    Alloca(Type),
    Load {
        ptr: Operand,
        ty: Type,
    },
    Store {
        ptr: Operand,
        val: Operand,
    },

    // Acesso a propriedades e posições
    GetField {
        object: Operand,
        index: usize,
    },
    SetField {
        object: Operand,
        index: usize,
        value: Operand,
    },
    GetListElement {
        list: Operand,
        index: Operand,
    },
    SetListElement {
        list: Operand,
        index: Operand,
        value: Operand,
    },
    CellGet {
        cell: Operand,
    },
    CellSet {
        cell: Operand,
        value: Operand,
    },
    EnvGet {
        env: Operand,
        index: usize,
    },

    // Chamadas
    CallStatic {
        symbol: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    CallInterface {
        receiver: Operand,
        selector_id: u32,
        selector_name: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    CallDynamic {
        receiver: Operand,
        selector_name: String,
        args: Vec<Operand>,
        ret_ty: Type,
    },
    /// Chamada de um valor função (closure, tear-off) pela convenção
    /// uniforme: todos os argumentos `Ref`, os posicionais primeiro e depois
    /// os nomeados na ordem de `nomes` (ordenados); o resultado é `Ref`. O
    /// emissor monta o vetor de argumentos e o descritor
    /// (`[n_posicionais, n_nomeados, hash(nome)…]`) e chama a entrada
    /// uniforme da closure pela tabela de código (`@df_code_table`).
    CallClosure {
        closure: Operand,
        args: Vec<Operand>,
        nomes: Vec<String>,
        ret_ty: Type,
    },
    CallRuntime {
        name: String,
        args: Vec<(Operand, Type)>,
        ret_ty: Type,
    },

    // Verificação e cast de tipo
    CheckNotNull(Operand),
    IsClass {
        object: Operand,
        class_id: u32,
    },

    // Globais do módulo (variáveis de topo e campos estáticos — N6)
    LoadGlobal {
        simbolo: String,
        ty: Type,
    },
    /// `raiz`: id do global `Ref`, que o runtime mantém como raiz
    /// permanente (N6/G6); `None` para a bandeira e para escalares.
    StoreGlobal {
        simbolo: String,
        val: Operand,
        ty: Type,
        raiz: Option<u32>,
    },

    // Phi node
    Phi {
        incoming: Vec<(BlockId, Operand)>,
        ty: Type,
    },

    // --- Closures (P1, docs/NATIVO-PLANO.md §7.4) -----------------------
    /// O tear-off canônico da função cuja entrada uniforme é `code_symbol`
    /// (o mesmo handle sempre: `identical(f, f)`).
    TearOff {
        code_symbol: String,
    },
    /// `base[index]` de um vetor de `i64` (argumentos ou descritor da
    /// convenção uniforme). O tipo registrado diz a representação lida
    /// (`Ref` para um argumento, `I64` para um campo do descritor).
    LoadIndexed {
        base: Operand,
        index: Operand,
    },
    /// Endereço (`Ptr`) de um vetor constante de `i64`, global do módulo
    /// (a assinatura de uma entrada uniforme para a checagem de aridade).
    ConstArray(Vec<i64>),
}

/// Terminador de controle de fluxo de um bloco básico.
#[derive(Debug, Clone)]
pub enum Terminator {
    Return(Option<Operand>),
    Branch(BlockId),
    CondBranch {
        cond: Operand,
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch {
        val: Operand,
        default: BlockId,
        cases: Vec<(i64, BlockId)>,
    },
    Throw(Operand),
    Unreachable,
}

/// Bloco básico em SSA.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<(ValueId, Instruction, Type)>,
    pub terminator: Terminator,
}

/// Função compilada na HIR.
#[derive(Debug, Clone)]
pub struct Function {
    pub symbol: String,
    pub name: String,
    pub params: Vec<(ValueId, String, Type)>,
    pub return_ty: Type,
    pub blocks: Vec<BasicBlock>,
}

/// Definição de classe na HIR.
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: u32,
    pub name: String,
    pub field_count: usize,
    /// Mapa de selector_id -> símbolo da função implementada
    pub vtable: Vec<(u32, String)>,
    pub to_string_symbol: Option<String>,
}

/// Seletor de chamada registrado globalmente.
#[derive(Debug, Clone)]
pub struct SelectorDef {
    pub id: u32,
    pub name: String,
    pub arity: usize,
}

/// Módulo HIR completo representando um programa Dart compilável.
#[derive(Debug, Default)]
pub struct Module {
    pub functions: Vec<Function>,
    pub classes: Vec<ClassDef>,
    pub selectors: Vec<SelectorDef>,
    pub subtyping_edges: Vec<(u32, u32)>,
    pub entry_symbol: Option<String>,
    /// Globais do usuário: (id da variável, representação). Cada um vira
    /// Globais do usuário: (id da raiz no runtime, representação, símbolo
    /// estável do valor `dfg.<caminho>`); a bandeira de inicialização é
    /// `<símbolo>$ok`.
    pub globais: Vec<(u32, Type, String)>,
    /// Construtos que o lowering não sabe baixar (N1). Não vazio = o
    /// programa não compila; o emissor produz só a mensagem.
    pub erros: Vec<String>,
}

impl Module {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra ou localiza um selector_id global para um nome e aridade.
    pub fn get_or_register_selector(&mut self, name: &str, arity: usize) -> u32 {
        if let Some(pos) = self.selectors.iter().position(|s| s.name == name && s.arity == arity) {
            return pos as u32;
        }
        let id = self.selectors.len() as u32;
        self.selectors.push(SelectorDef {
            id,
            name: name.to_string(),
            arity,
        });
        id
    }
}

