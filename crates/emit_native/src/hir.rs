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
    /// Bytes WTF-8 de um literal Dart; preserva surrogates isolados.
    StringWtf8(Vec<u8>),
    Null,
    /// O endereço da função `símbolo` como `i64` (`ptrtoint`): a mesma em
    /// todos os isolados do processo.
    Funcao(String),
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

    // --- P5c (SDK da fonte, δ) ------------------------------------------
    /// Chamada de membro de instância pelo **seletor** (`c:m`, `g:x`,
    /// `s:x`, `lower/sdk_fonte.rs`), no mundo aberto: a implementação sai da
    /// tabela de métodos da classe dinâmica do receptor (registrada no
    /// runtime por classe), com um cache por ponto de chamada. A convenção é
    /// a uniforme das closures: todos os argumentos `Ref`, posicionais e
    /// depois nomeados na ordem de `nomes`; o resultado é `Ref`. Receptor sem
    /// o membro: `NoSuchMethodError` pendente.
    CallSeletor {
        seletor: String,
        recv: Operand,
        args: Vec<Operand>,
        nomes: Vec<String>,
        /// Tupla RTI do método genérico, no slot oculto após os argumentos.
        tupla_tipos: Operand,
    },
    /// Chama o valor função `closure` repassando o vetor de argumentos e o
    /// descritor já montados (os parâmetros de uma entrada uniforme): o
    /// adaptador `c:x` de um getter ou campo chama o valor lido.
    CallClosureRepasse {
        closure: Operand,
        args: Operand,
        desc: Operand,
    },
    /// Chamada a uma função nativa (C) no endereço `alvo` (`i64`), com a
    /// ABI C do alvo (`dart:ffi`, `lower/ffi.rs`). Os argumentos vêm na
    /// representação da HIR — inteiros e ponteiros `I64`, ponto flutuante
    /// `F64`, `bool` `I1` — e são convertidos ao tipo C; o resultado volta
    /// do mesmo jeito (inteiros estendidos a 64 bits pelo sinal do tipo C).
    ChamadaNativa {
        alvo: Operand,
        args: Vec<(Operand, TipoC)>,
        ret: TipoC,
    },
}

/// Um tipo C na fronteira de uma chamada nativa (os primitivos do
/// `dart:ffi`, com os inteiros específicos da ABI já resolvidos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoC {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Bool,
    Ptr,
    Void,
}

impl TipoC {
    /// O tipo LLVM do valor C.
    pub fn llvm(self) -> &'static str {
        match self {
            TipoC::I8 | TipoC::U8 => "i8",
            TipoC::I16 | TipoC::U16 => "i16",
            TipoC::I32 | TipoC::U32 => "i32",
            TipoC::I64 | TipoC::U64 => "i64",
            TipoC::F32 => "float",
            TipoC::F64 => "double",
            TipoC::Bool => "i1",
            TipoC::Ptr => "ptr",
            TipoC::Void => "void",
        }
    }

    /// A representação na HIR do valor Dart correspondente.
    pub fn tipo_hir(self) -> Type {
        match self {
            TipoC::F32 | TipoC::F64 => Type::F64,
            TipoC::Bool => Type::I1,
            TipoC::Void => Type::Void,
            _ => Type::I64,
        }
    }

    /// O atributo de extensão de um parâmetro estreito (quem chama estende:
    /// exigido pela ABI da Apple em arm64 e inócuo nas outras).
    pub fn extensao(self) -> &'static str {
        match self {
            TipoC::I8 | TipoC::I16 => "signext ",
            TipoC::U8 | TipoC::U16 | TipoC::Bool => "zeroext ",
            _ => "",
        }
    }

    /// A letra do tipo na chave de uma assinatura nativa.
    pub fn letra(self) -> char {
        match self {
            TipoC::I8 => 'a',
            TipoC::U8 => 'h',
            TipoC::I16 => 's',
            TipoC::U16 => 't',
            TipoC::I32 => 'i',
            TipoC::U32 => 'j',
            TipoC::I64 => 'l',
            TipoC::U64 => 'm',
            TipoC::F32 => 'f',
            TipoC::F64 => 'd',
            TipoC::Bool => 'b',
            TipoC::Ptr => 'p',
            TipoC::Void => 'v',
        }
    }
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
    /// `dart:ffi`: (id RTI da classe, letra do tipo C) de cada tipo nativo
    /// e (chave da assinatura, símbolo do trampolim) (`lower/ffi.rs`).
    pub ffi_tipos: Vec<(i64, char)>,
    pub ffi_trampolins: Vec<(String, String)>,
    pub functions: Vec<Function>,
    pub classes: Vec<ClassDef>,
    pub selectors: Vec<SelectorDef>,
    pub subtyping_edges: Vec<(u32, u32)>,
    pub entry_symbol: Option<String>,
    /// Quantos parâmetros o `main` declara: com um, recebe os argumentos da
    /// linha de comando (`List<String>`); o segundo (`message`, do
    /// `Isolate.spawnUri`) é `null`.
    pub entry_params: usize,
    /// Globais do usuário: (id da variável, representação). Cada um vira
    /// Globais do usuário: (id da raiz no runtime, representação, símbolo
    /// estável do valor `dfg.<caminho>`); a bandeira de inicialização é
    /// `<símbolo>$ok`.
    pub globais: Vec<(u32, Type, String)>,
    /// Construtos que o lowering não sabe baixar (N1). Não vazio = o
    /// programa não compila; o emissor produz só a mensagem.
    pub erros: Vec<String>,
    // --- P5c (SDK da fonte, δ) ---
    /// O módulo é parte de um programa com o SDK compilado da fonte: o
    /// código compartilhado entre módulos (entradas de tear-off, constantes
    /// canônicas) sai em `comdat`, e símbolos de outros módulos são
    /// declarados.
    pub modo_sdk: bool,
    /// O módulo é uma biblioteca do SDK (sem `dartforge_entry` nem o
    /// despacho de `toString`, que são do programa).
    pub biblioteca_sdk: bool,
    /// Tabelas de métodos das classes deste módulo, registradas no runtime
    /// na partida (P5c): (id da classe, [(seletor, símbolo do adaptador)]).
    pub tabelas_de_metodos: Vec<(u32, String, Vec<(String, String)>)>,
    /// A função que devolve a tabela de métodos de cada classe concreta
    /// compilada (de qualquer módulo), pelo id: a alocação de um objeto da
    /// classe registra a tabela (`dartforge_object_new_t`).
    pub funcoes_de_tabela: std::collections::HashMap<u32, String>,
    /// Biblioteca do SDK: o nome da função que registra as classes dela
    /// (nomes, subtipos, tabelas de métodos) no runtime.
    pub registro: Option<String>,
    /// Programa com o SDK da fonte: as funções de registro das bibliotecas
    /// do SDK, chamadas por `dartforge_entry` antes das do programa.
    pub registros_do_sdk: Vec<String>,
    /// Programa com o SDK da fonte: os ids de classe dos valores que o
    /// runtime representa (`Null`, `_Smi`, `_Mint`, `_Double`, `bool`,
    /// `_OneByteString`, `_TwoByteString`, `_GrowableList`, `_List`,
    /// `_ImmutableList`, `_Closure`, `_Record`), na ordem de
    /// `runtime/src/seletores.rs`.
    pub cids_do_runtime: Vec<i64>,
    /// Programa com o SDK da fonte: a versão do SDK Dart compilado (o
    /// arquivo `version` dele), que `Platform.version` informa.
    pub versao_do_sdk: Option<String>,
    /// Membros do SDK da fonte recusados neste módulo: (símbolo, motivo).
    /// Cada um virou uma função que avisa em tempo de execução.
    pub recusados: Vec<(String, String)>,
    /// Funções Dart que o runtime chama pelo nome (`_dartforge*` da
    /// sobreposição): (nome, símbolo), registradas pelo registro do módulo.
    pub ajudantes: Vec<(String, String)>,
    // --- P6 (bibliotecas da fonte, `fonte.rs`) ---------------------------
    /// Diagnósticos das funções de bibliotecas do SDK compiladas da fonte,
    /// pelo símbolo da função que os produziu: só viram `erros` se a poda
    /// (`fonte::podar`) mantiver a função.
    pub erros_da_fonte: Vec<(String, Vec<String>)>,
    /// Funções da fonte que o runtime chama sem que o programa as referencie
    /// (raízes da poda, além das funções do programa).
    pub raizes_da_fonte: Vec<String>,
    /// A função que o laço de eventos do runtime usa para chamar uma
    /// closure sem argumentos (`dartforge_laco_de_eventos`, depois do
    /// `main`). `None`: o programa não usa `dart:async` e não há laço.
    pub chamar_dart: Option<String>,
    /// RTI: a função que registra o universo de tipos antes do `main`
    /// (`lower::rti::registrar_universo`); `None` sem receitas.
    pub iniciar_rti: Option<String>,
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
