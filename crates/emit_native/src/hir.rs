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
    /// `Float32x4` sem caixa: `<4 x float>` (`docs/SIMD-NATIVO.md`). Só em
    /// locais e temporários de funções síncronas; nas fronteiras (campos,
    /// parâmetros, retornos, coleções) o valor é a caixa `Ref`.
    V4F32,
    /// `Int32x4` sem caixa: `<4 x i32>`.
    V4I32,
    /// `Float64x2` sem caixa: `<2 x double>`.
    V2F64,
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
            Self::V4F32 => "<4 x float>",
            Self::V4I32 => "<4 x i32>",
            Self::V2F64 => "<2 x double>",
        }
    }

    /// Um dos vetores SIMD sem caixa?
    pub fn is_vector(self) -> bool {
        matches!(self, Self::V4F32 | Self::V4I32 | Self::V2F64)
    }
}

/// Uma operação SIMD sobre vetores sem caixa, com a semântica pista a pista
/// da VM (a mesma de `runtime/src/simd.rs`). O tipo do vetor é o da
/// instrução ou o do primeiro operando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpSimd {
    /// `a op b` pista a pista (`+ - * /` nos de ponto flutuante; `+ -` com
    /// volta no estouro e `& | ^` no `Int32x4`).
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
    Xor,
    /// `a < b ? a : b` e `a > b ? a : b` (o `Utils::Minimum`/`Maximum` da
    /// VM: com NaN, o segundo operando).
    Min,
    Max,
    /// `max(min(a, hi), lo)`.
    Clamp,
    Neg,
    Abs,
    Sqrt,
    /// `1 / x` e `sqrt(1 / x)`.
    Recip,
    RecipSqrt,
    /// `v * s`, com `s` `double` (arredondado a `float` no `Float32x4`).
    Scale,
    /// Comparação de `Float32x4` que dá a máscara `Int32x4` (-1/0).
    Cmp(FCmpOp),
    /// A pista `i` como `double` (`int` no `Int32x4`).
    Hint(u8),
    /// O vetor com a pista `i` trocada.
    WithHint(u8),
    /// A pista `i` do `Int32x4` como `bool` (≠ 0).
    Flag(u8),
    /// O `Int32x4` com a pista `i` em -1/0 pelo `bool`.
    WithFlag(u8),
    /// O vetor das pistas dadas (`double`, `int` ou `bool`).
    Builds,
    /// `Int32x4.bool(x, y, z, w)`: -1/0 por pista.
    BuildsFlags,
    Splat,
    Zero,
    /// Os bits de sinal das pistas, a pista 0 no bit 0.
    SignMask,
    /// `shuffle(m)` / `shuffleMix(b, m)` com a máscara constante `m`.
    Shuffle(u8),
    ShuffleMix(u8),
    /// `Int32x4.select(t, f)`: bit a bit, `t` onde a máscara tem 1.
    Select,
    /// Os mesmos bits noutro tipo (`fromInt32x4Bits`, `fromFloat32x4Bits`).
    Bits,
    /// `Float64x2.fromFloat32x4` (pistas 0 e 1) e `Float32x4.fromFloat64x2`
    /// (`[x, y, 0, 0]`).
    Converts,
    /// O vetor no endereço `args[0]` + 16 × `args[1]` (lista SIMD).
    Loads,
    /// Grava `args[2]` no endereço `args[0]` + 16 × `args[1]`.
    Writes,
    // API de `Int32x4` do Dart 3.14 (`docs/SIMD-NATIVO.md` §6), pela
    // extensão marcada `@pragma('dartforge:simd-api', '3.14')`. `Mul`,
    // `Min`, `Max`, `Neg` e `Abs` servem também ao `Int32x4` (inteiro, com
    // volta no estouro; `abs(-2^31)` é `-2^31`).
    /// Comparação com sinal pista a pista de `Int32x4`: máscara -1/0.
    CmpInt(ICmpOp),
    /// `a & ~b`.
    AndNot,
    /// `~a`.
    Not,
    /// `a << (s & 31)` (`false`) ou `a >> (s & 31)` aritmético (`true`),
    /// com `s` `int`.
    Shifts(bool),
    /// Alguma pista ≠ 0 (`anyTrue`) / todas ≠ 0 (`allTrue`): `bool`.
    Any,
    All,
    /// `Float32x4(v.x.toDouble(), v.y.toDouble(), v.z.toDouble(),
    /// v.w.toDouble())` de one `Int32x4` `v` (`lower/simd.rs`): `sitofp`
    /// pista a pista — o mesmo valor, pois o `int` de 32 bits vira `double`
    /// sem perda e o `double` vira `float` com um arredondamento só.
    IntToFloat,
    /// `fptosi` pista a pista de um `Float32x4` cujas pistas estão todas em
    /// `[-2^31, 2^31)` (conferido antes por [`OpSimd::InInt32Range`]): o
    /// `toInt()` de cada pista, que aí não lança nem satura.
    FloatToInt,
    /// Todas as pistas do `Float32x4` em `[-2^31, 2^31)` (NaN não está):
    /// `bool`.
    InInt32Range,
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
    Function(String),
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
    /// Menor sem sinal: `i u< n` testa `0 <= i < n` de uma vez.
    Ult,
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

/// Slot proprietário classificado; não aceita ponteiro nativo arbitrário.
///
/// Quadros são IDs escalares da ABI ARC, com slots Ref inicialmente nulos.
/// O runtime confere identidade, propriedade e limite do índice.
/// Slots de heap/nativos ainda exigem seus próprios descritores.
///
/// ```
/// use dartforge_emit_native::hir::*;
/// let slot = SlotForte::Quadro { quadro: Operand::Val(ValueId(0)), indice: 2 };
/// assert!(matches!(slot, SlotForte::Quadro { indice: 2, .. }));
/// ```
#[derive(Debug, Clone)]
pub enum StrongSlot {
    /// Slot Ref num quadro proprietário aberto pela ABI, identificado por SSA I64.
    Quadro { frame: Operand, index: u32 },
    /// Armazenamento global Ref declarado exatamente uma vez no módulo.
    /// Inicialização lazy continua sendo responsabilidade do lowering.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = SlotForte::Global { simbolo: "dfg.recurso".into() };
    /// assert!(matches!(slot, SlotForte::Global { .. }));
    /// ```
    Global { symbol: String },
}

/// Como publicar uma referência no slot proprietário.
///
/// ```
/// use dartforge_emit_native::hir::ModoStoreForte;
/// assert_ne!(ModoStoreForte::Copy, ModoStoreForte::Move);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrongStoreMode {
    /// Retém uma ocorrência independente; não consome o token SSA.
    Copy,
    /// Consome o token SSA no sucesso, transferindo sua ocorrência.
    Move,
}

/// Instruções que produzem um valor (ou efetuam efeito de escrita).
#[derive(Debug, Clone)]
pub enum Instruction {
    /// Carrega uma constante em um identificador SSA.
    Const(Constant),

    /// Copia uma referência já avaliada e produz token owned independente.
    /// Retém sem coletar. O operando é SSA Ref ou null; não cria caixas.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let copia = Instruction::ArcCopy { value: Operand::Val(ValueId(0)) };
    /// assert!(matches!(copia, Instruction::ArcCopy { .. }));
    /// ```
    ArcCopy { value: Operand },
    /// Consome um token owned, sem coletar ou executar Dart.
    /// O verificador de tokens deverá provar disponibilidade e consumo único.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let drop = Instruction::ArcDrop { value: Operand::Val(ValueId(1)) };
    /// assert!(matches!(drop, Instruction::ArcDrop { .. }));
    /// ```
    ArcDrop { value: Operand },
    /// Transfere o token para o resultado Ref sem alterar RC físico.
    /// Consome a origem lógica; preservar a operação até verificar ownership.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let movimento = Instruction::ArcMove { value: Operand::Val(ValueId(1)) };
    /// assert!(matches!(movimento, Instruction::ArcMove { .. }));
    /// ```
    ArcMove { value: Operand },

    /// Carrega slot classificado e produz token owned independente do slot.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = SlotForte::Quadro { quadro: Operand::Val(ValueId(0)), indice: 0 };
    /// let carga = Instruction::ArcLoadStrong { slot };
    /// assert!(matches!(carga, Instruction::ArcLoadStrong { .. }));
    /// ```
    ArcLoadStrong { slot: StrongSlot },
    /// Publica a referência já avaliada antes de liberar o conteúdo antigo.
    ///
    /// ```
    /// use dartforge_emit_native::hir::*;
    /// let slot = SlotForte::Quadro { quadro: Operand::Val(ValueId(0)), indice: 0 };
    /// let store = Instruction::ArcStoreStrong {
    ///     slot, value: Operand::Constant(Constant::Null), modo: ModoStoreForte::Copy,
    /// };
    /// assert!(matches!(store, Instruction::ArcStoreStrong { .. }));
    /// ```
    ArcStoreStrong { slot: StrongSlot, value: Operand, mode: StrongStoreMode },

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
    AllocRecord {
        elements: Vec<(Operand, u8)>,
    },
    AllocCell {
        value: Operand,
    },
    AllocEnv {
        values: Vec<Operand>,
    },
    /// A interpolação `'a$b c'`: as partes numa só string, com uma alocação
    /// (`dartforge_string_juntar_tipado`), não uma por concatenação. Cada
    /// parte é um texto (`Ref`) ou um `int` sem caixa (`I64`), escrito em
    /// decimal direto no resultado.
    ConcatStrings {
        parts: Vec<Operand>,
    },
    AllocClosure {
        code_symbol: String,
        env: Operand,
    },
    /// Closure cujo corpo tem ABI tipada (`lower/closures.rs`): além da
    /// entrada uniforme `code_symbol`, o corpo `typed`
    /// `(env, p0…) -> r` nas representações da HIR, com o código `abi`
    /// delas, que a chamada tipada confere.
    AllocClosureTyped {
        code_symbol: String,
        env: Operand,
        typed: String,
        abi: i64,
        /// `env` é o único valor capturado, guardado no lugar do ambiente
        /// (`lower/closures.rs`, ambiente direto): não é um ambiente.
        direct: bool,
    },
    /// Chamada do corpo tipado de uma closure: `alvo` é o endereço (`I64`,
    /// de `dartforge_closure_tipada`), os argumentos vão nas representações
    /// da HIR (o primeiro é o ambiente) e o resultado volta em `ret`. A
    /// exceção sai pela pendência, como numa chamada Dart.
    TypedCall {
        alvo: Operand,
        args: Vec<(Operand, Type)>,
        ret: Type,
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
    /// os nomeados na ordem de `names` (ordenados); o resultado é `Ref`. O
    /// emissor monta o vetor de argumentos e o descritor
    /// (`[n_posicionais, n_nomeados, hash(name)…]`) e chama a entrada
    /// uniforme da closure pelo endereço guardado nela (`@df_clo_invalido`
    /// quando o valor não é closure). Como na chamada por seletor, o vetor
    /// tem um slot a mais, depois dos argumentos, com a tupla RTI dos
    /// argumentos de tipo (`tuple_types`, `I64`; `0` quando a chamada não
    /// passa nenhum): é dela que uma closure genérica lê os seus.
    CallClosure {
        closure: Operand,
        args: Vec<Operand>,
        names: Vec<String>,
        ret_ty: Type,
        tuple_types: Operand,
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
        symbol: String,
        ty: Type,
    },
    /// `root`: id do global `Ref`, que o runtime mantém como raiz
    /// permanente (N6/G6); `None` para a bandeira e para escalares.
    StoreGlobal {
        symbol: String,
        val: Operand,
        ty: Type,
        root: Option<u32>,
    },

    /// Operação SIMD sem caixa (`OpSimd`); o tipo do resultado é o da
    /// instrução.
    Simd {
        op: OpSimd,
        args: Vec<Operand>,
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
    /// imutável: assinaturas uniformes e tabelas de layouts físicos ARC.
    /// As palavras são dados nativos; não constituem slots Ref do heap.
    ConstArray(Vec<i64>),
    /// Endereço (`Ptr`) de um vetor constante com o endereço de cada função
    /// (`@df.fns.<k>`, global do módulo): os getters das constantes de um
    /// literal grande que vira tabela (`lower/literals.rs`,
    /// `fill_from_table`), que o runtime chama.
    FunctionTable(Vec<String>),

    // --- P5c (SDK da fonte, δ) ------------------------------------------
    /// Chamada de membro de instância pelo **seletor** (`c:m`, `g:x`,
    /// `s:x`, `lower/sdk_source.rs`), no mundo aberto: a implementação sai da
    /// tabela de métodos da classe dinâmica do receptor (registrada no
    /// runtime por classe), com um cache por ponto de chamada. A convenção é
    /// a uniforme das closures: todos os argumentos `Ref`, posicionais e
    /// depois nomeados na ordem de `names`; o resultado é `Ref`. Receptor sem
    /// o membro: `NoSuchMethodError` pendente.
    CallSelector {
        selector: String,
        recv: Operand,
        args: Vec<Operand>,
        names: Vec<String>,
        /// Tupla RTI do método genérico, no slot oculto após os argumentos.
        tuple_types: Operand,
    },
    /// Chama o valor função `closure` repassando o vetor de argumentos e o
    /// descritor já montados (os parâmetros de uma entrada uniforme): o
    /// adaptador `c:x` de um getter ou campo chama o valor lido.
    CallClosureForward {
        /// Retorno Owned pelo adaptador ARC, com entrada uniforme certificada.
        arc_owned: bool,
        closure: Operand,
        args: Operand,
        desc: Operand,
    },
    /// `recv.<selector>` repassando o vetor de argumentos e o descritor já
    /// montados (os de uma entrada uniforme, com a tupla de tipos no slot
    /// depois dos argumentos): a entrada do tear-off de um método chama a
    /// implementação do receptor, que confere os argumentos e completa os
    /// opcionais com os padrões dela. Resultado `Ref`.
    CallSelectorForward {
        /// Usa o adaptador ARC com retorno Owned e guarda de pendência.
        /// Só pode ser emitida junto ao inventário AOT certificado.
        arc_owned: bool,
        selector: String,
        recv: Operand,
        args: Operand,
        desc: Operand,
    },
    /// Chamada a uma função nativa (C) no endereço `alvo` (`i64`), com a
    /// ABI C do alvo (`dart:ffi`, `lower/ffi.rs`). Os argumentos vêm na
    /// representação da HIR — inteiros e ponteiros `I64`, ponto flutuante
    /// `F64`, `bool` `I1` — e são convertidos ao tipo C; o resultado volta
    /// do mesmo jeito (inteiros estendidos a 64 bits pelo sinal do tipo C).
    NativeCall {
        alvo: Operand,
        args: Vec<(Operand, CType)>,
        ret: CType,
    },
    /// O elemento `index` (tipo C `ty`) da memória em `address` (`I64`):
    /// o caminho rápido de uma lista tipada, com os limites já conferidos.
    /// O resultado vem na representação Dart do tipo (`I64`, `F64`).
    NativeLoad {
        address: Operand,
        index: Operand,
        ty: CType,
    },
    /// Grava `value` (representação Dart) como o elemento `index` do tipo
    /// C `ty` em `address`.
    NativeWrite {
        address: Operand,
        index: Operand,
        ty: CType,
        value: Operand,
    },
    /// Uma chamada nativa com structs ou unions por valor, ou variádica: o operando de um
    /// composto é o endereço (`I64`) dos bytes dele, e um retorno composto é
    /// gravado em `dest` (o resultado da instrução é então `Void`). O
    /// emissor aplica a ABI C do alvo (`llvm/c_abi.rs`).
    CompositeNativeCall {
        alvo: Operand,
        args: Vec<(Operand, NativeType)>,
        ret: NativeType,
        dest: Option<Operand>,
        /// Função variádica (`VarArgs`): quantos parâmetros são fixos; os
        /// seguintes vão com as promoções de argumento do C.
        variadic_flag: Option<usize>,
    },
}

/// Um tipo na fronteira nativa: primitivo ou composto por valor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NativeType {
    Prim(CType),
    Composite(LayoutC),
}

/// O layout C de uma struct ou union: tamanho, alinhamento e as folhas
/// primitivas (deslocamento, tipo), com structs aninhadas e arrays
/// achatados — o que a classificação da ABI examina.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayoutC {
    pub size: usize,
    pub alignment: usize,
    pub leaves: Vec<(usize, CType)>,
}

impl LayoutC {
    /// Alguma folha fora do alinhamento natural (`@Packed`).
    pub fn misaligned(&self) -> bool {
        self.leaves.iter().any(|(o, t)| {
            let n = t.c_size();
            n > 1 && o % n != 0
        })
    }
}

impl NativeType {
    /// A representação na HIR do valor Dart correspondente (um composto
    /// chega como endereço).
    pub fn hir_type(&self) -> Type {
        match self {
            NativeType::Prim(t) => t.hir_type(),
            NativeType::Composite(_) => Type::I64,
        }
    }
}

/// Um tipo C na fronteira de uma chamada nativa (os primitivos do
/// `dart:ffi`, com os inteiros específicos da ABI já resolvidos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CType {
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
    /// `Handle`: um objeto Dart na fronteira nativa, como o ponteiro opaco
    /// de uma célula do runtime (o `Dart_Handle` da VM).
    Handle,
    Void,
}

impl CType {
    /// O tipo LLVM do valor C.
    pub fn llvm(self) -> &'static str {
        match self {
            CType::I8 | CType::U8 => "i8",
            CType::I16 | CType::U16 => "i16",
            CType::I32 | CType::U32 => "i32",
            CType::I64 | CType::U64 => "i64",
            CType::F32 => "float",
            CType::F64 => "double",
            CType::Bool => "i1",
            CType::Ptr | CType::Handle => "ptr",
            CType::Void => "void",
        }
    }

    /// A representação na HIR do valor Dart correspondente.
    pub fn hir_type(self) -> Type {
        match self {
            CType::F32 | CType::F64 => Type::F64,
            CType::Bool => Type::I1,
            CType::Void => Type::Void,
            _ => Type::I64,
        }
    }

    /// O atributo de extensão de um parâmetro estreito (quem chama estende:
    /// exigido pela ABI da Apple em arm64 e inócuo nas outras).
    pub fn extension(self) -> &'static str {
        match self {
            CType::I8 | CType::I16 => "signext ",
            CType::U8 | CType::U16 | CType::Bool => "zeroext ",
            _ => "",
        }
    }

    /// Bytes do tipo C (0 para `void`).
    pub fn c_size(self) -> usize {
        match self {
            CType::I8 | CType::U8 | CType::Bool => 1,
            CType::I16 | CType::U16 => 2,
            CType::I32 | CType::U32 | CType::F32 => 4,
            CType::I64 | CType::U64 | CType::F64 | CType::Ptr | CType::Handle => 8,
            CType::Void => 0,
        }
    }

    /// A letra do tipo na chave de uma assinatura nativa.
    pub fn letter(self) -> char {
        match self {
            CType::I8 => 'a',
            CType::U8 => 'h',
            CType::I16 => 's',
            CType::U16 => 't',
            CType::I32 => 'i',
            CType::U32 => 'j',
            CType::I64 => 'l',
            CType::U64 => 'm',
            CType::F32 => 'f',
            CType::F64 => 'd',
            CType::Bool => 'b',
            CType::Ptr => 'p',
            CType::Handle => 'H',
            CType::Void => 'v',
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
    /// As posições na fonte, quando a compilação pede informação de
    /// depuração (`CompileOptions::debug_info`, J05).
    pub debug_info: Option<Box<FunctionDebugInfo>>,
}

/// A fonte de uma função e a posição de cada instrução (a do comando que a
/// gerou), para as tabelas de linha do depurador (`llvm/debug_info.rs`).
/// Instrução sem posição (criada por um passo de otimização) herda a da
/// anterior no bloco.
#[derive(Debug, Clone, Default)]
pub struct FunctionDebugInfo {
    /// O caminho absoluto do arquivo `.dart` (ou o URI, sem arquivo).
    pub file: String,
    /// A url do script no rastro da VM (§13.14, `Context::stack_trace_url`).
    pub url: String,
    /// A linha do primeiro comando com posição (a do `DISubprogram`).
    pub line: u32,
    /// `(line, column)` de cada instrução, a partir de 1.
    pub positions: std::collections::HashMap<ValueId, (u32, u32)>,
    /// A do terminador de cada bloco (o `return` e o `break` não emitem
    /// instrução).
    pub exits: std::collections::HashMap<BlockId, (u32, u32)>,
    // --- o rastro no formato da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14)
    /// A posição do token da função (o `Function.token_pos` da VM: o nome,
    /// ou o começo de uma closure): a do quadro de uma closure que escuta um
    /// `Future` (`kFutureListenerPcOffset`).
    pub token: (u32, u32),
    /// As funções copiadas nesta pelo inlining da HIR (`optimize::inline`),
    /// para o rastro mostrar o quadro de cada uma como a VM mostra os quadros
    /// embutidos (`GetInlinedFunctionsAtReturnAddress`).
    pub embedded_set: Vec<Embedded>,
    /// `(line, column, context)` das instruções copiadas pelo inlining: a
    /// posição na função copiada e o índice dela em `embedded_set`. Fica fora de
    /// `positions`, que é das tabelas de linha (o arquivo é o desta função).
    pub embedded_positions: std::collections::HashMap<ValueId, (u32, u32, u32)>,
    /// As marcas do rastro ([`stack_trace_marks`]).
    pub marks: u8,
    /// Corpo `async` (a máquina de estados): a entrada das closures dele e a
    /// posição de cada `await`.
    pub async_body: Option<TraceAsyncBody>,
    /// A entrada uniforme (`$ent`), quando esta função é o corpo de uma
    /// closure: o código que a closure guarda, pelo qual o runtime acha a
    /// função de uma closure que escuta um `Future`.
    pub closure_entry: Option<String>,
    /// A captura marcada `@pragma('vm:awaiter-link')` de uma closure: o elo
    /// com quem espera (`ClosureData::awaiter_link` da VM).
    pub chain_link: Option<WaitLink>,
}

impl Function {
    /// O nome da função no rastro no formato da VM (o
    /// `QualifiedUserVisibleName`): o qualificado do Dart, tirado do símbolo
    /// estável `df.<library>.<class>.<member>`, com as partes que o
    /// lowering acrescenta trocadas como a VM as escreve: `$clo<k>` vira
    /// `.<anonymous closure>`, `$<name>` de função local vira `.<name>`, e as
    /// impressões digitais e os sufixos de corpo (`$e…`, `$q…`, `$async`,
    /// `$ent`, `$new`, a repetição `$<k>`) saem.
    pub fn stack_trace_name(&self) -> String {
        let base = match self.symbol.strip_prefix("dart_main") {
            Some(rest) => format!("main{rest}"),
            None => self
                .symbol
                .strip_prefix("df.")
                .and_then(|rest| rest.split_once('.'))
                .map(|(_, name)| name.trim_start_matches('.').to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| self.name.clone()),
        };
        let printing = |s: &str, p: char| s.len() == 9 && s.starts_with(p) && s[1..].bytes().all(|b| b.is_ascii_hexdigit());
        let mut parts = base.split('$');
        let mut name = parts.next().unwrap_or_default().to_string();
        for p in parts {
            if p.is_empty() || p == "async" || p == "ent" || p == "novo" || printing(p, 'e') || printing(p, 'q') || p.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if let Some(k) = p.strip_prefix("clo")
                && !k.is_empty()
                && k.bytes().all(|b| b.is_ascii_digit())
            {
                name.push_str(".<anonymous closure>");
                continue;
            }
            name.push('.');
            name.push_str(p);
        }
        name
    }
}

/// Uma função copiada pelo inlining da HIR.
#[derive(Debug, Clone)]
pub struct Embedded {
    /// O nome da função copiada, como o rastro o escreve.
    pub name: String,
    /// A url do script dela.
    pub url: String,
    /// A posição da chamada que a copiou, na função de fora (o contexto
    /// `parent`, ou a própria função).
    pub call: (u32, u32),
    /// O contexto de fora (índice em `embedded_set`); `None`: a própria função.
    pub parent: Option<u32>,
    /// As marcas do rastro da função copiada.
    pub marks: u8,
}

/// A parte do rastro de um corpo `async`.
#[derive(Debug, Clone, Default)]
pub struct TraceAsyncBody {
    /// A entrada uniforme do corpo (o código da closure do corpo, cujo
    /// ambiente guarda o quadro na posição 0).
    pub entry: String,
    /// A posição do `await` de cada estado: o índice `k − 1` para o estado `k`.
    pub waits: Vec<(u32, u32)>,
}

/// Onde mora a captura que é o elo com quem espera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitLink {
    /// A posição no ambiente (sem `direct`).
    pub index: usize,
    /// O ambiente é o próprio valor capturado (`lower/closures.rs`).
    pub direct: bool,
    /// A captura mora numa célula.
    pub cell: bool,
}

/// As marcas do rastro de uma função ([`FunctionDebugInfo::marks`]).
pub mod stack_trace_marks {
    /// O corpo de uma função `async` (a máquina de estados): o quadro dele,
    /// retomado, começa a cadeia de quem espera.
    pub const ASYNC_BODY: u8 = 1;
    /// Fora do rastro: o stub de uma função `async` (a VM tem um quadro só
    /// para ela) e os apoios do `async_patch` (`@pragma("dartforge:stack_trace-oculto")`).
    pub const HIDDEN: u8 = 2;
    /// `_FutureListener.handleValue`: o ouvinte dela começa a cadeia
    /// (`MethodRecognizer::kFutureListenerHandleValue`).
    pub const LISTEN: u8 = 4;
    /// Com [`ASYNC_BODY`], o corpo de um gerador `async*`: o quadro dele na
    /// pilha conta sempre como já suspenso (`WasPreviouslySuspended`), e o
    /// próximo da cadeia é o controlador.
    pub const ASYNC_GENERATOR: u8 = 8;
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

/// Uma struct/union do programa para o runtime: a classe (RTI e heap), onde
/// ficam `_typedDataBase`/`_offsetInBytes` no objeto, e a medida na ABI.
#[derive(Debug, Clone)]
pub struct FfiComposite {
    pub rti: i64,
    pub class: i64,
    pub fields: i64,
    pub base_index: i64,
    pub offset_index: i64,
    pub size: i64,
    pub alignment: i64,
}

/// Um callback nativo de uma assinatura: a chave, o corpo HIR (contexto e
/// argumentos na representação Dart → retorno Dart; um composto vai e volta
/// como o endereço dos bytes) e os tipos nativos.
#[derive(Debug, Clone)]
pub struct FfiCallback {
    pub key: String,
    pub body: String,
    pub ret: NativeType,
    pub params: Vec<NativeType>,
}

/// Um campo do layout de uma classe do programa (J03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutField {
    pub name: String,
    /// `null` é um valor do tipo declarado (`T?`, `dynamic`, `Object?`…).
    pub nullable: bool,
    pub late: bool,
    /// O tipo declarado, como texto (uma mudança dele não migra).
    pub ty: String,
}

/// Como um `Return` sai quando a exceção pode estar pendente, nas exceções por
/// tabelas (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): uma função Dart
/// nunca volta a quem a chamou com a exceção pendente — ela desenrola.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionExit {
    /// A exceção está pendente em todo caminho até este `Return`: desenrola
    /// (`@df.throw`) em vez de retornar.
    Throws,
    /// Retoma o objeto nativo recebido neste pouso de cleanup Itanium.
    /// Só admite drops ARC e fechamento LIFO de quadros locais antes do Return.
    /// Não coleta, chama Dart nem publica uma nova exceção.
    Resume,
    /// Pode estar pendente: confere a pendência e desenrola ou retorna.
    Guard,
}

/// O que o passe das exceções por tabelas (`optimize/tables.rs`) decidiu
/// para uma função: quem é `invoke`, onde pousa e como cada `Return` sai. O
/// emissor (`llvm/mod.rs`) só lê.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunctionTables {
    /// A instrução (chamada Dart) que vira `invoke`, e o bloco de pouso dela.
    pub invocations: std::collections::HashMap<ValueId, BlockId>,
    /// Os blocos de pouso: começam com o `landingpad` e a restauração do
    /// topo da pilha-sombra; só o `invoke` chega a eles.
    pub landing_pads: std::collections::HashSet<BlockId>,
    /// Os blocos que terminam em `Return` com a exceção (talvez) pendente.
    pub exits: std::collections::HashMap<BlockId, ExceptionExit>,
    /// A função chama código Dart: confere a pilha no prólogo
    /// (`StackOverflowError`). No modelo de conferência a leitura da
    /// pendência depois de cada chamada já dava à função o contexto em que a
    /// conferência da pilha mora; aqui a leitura some, e sem esta marca
    /// `f() => f()` estouraria a pilha do sistema.
    pub checks_stack: bool,
    /// Inventário certificado de owners para cleanup estrangeiro; recalcular após mutações.
    pub foreign_cleanup: Option<crate::optimize::arc::ForeignCleanup>,
    /// Papéis nativos de quadros compartilhados para conferir Retoma/cleanup.
    /// Não substitui o certificado de owners nem prova unwind por si só.
    pub shared_frames: Option<crate::optimize::arc::SharedFramesCfgContract>,
}

/// Papel semântico dos vetores nativos da ABI de entrada uniforme.
/// O descritor contém contagens/hashes; argumentos podem conter handles Ref.
/// O endereço do vetor não é um handle nem um slot proprietário do heap.
///
/// ```
/// use dartforge_emit_native::hir::VetorEntradaDart;
/// assert_ne!(VetorEntradaDart::Argumentos, VetorEntradaDart::Descritor);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DartEntryVector {
    Argumentos,
    Descriptor,
}

/// Módulo HIR completo representando um programa Dart compilável.
#[derive(Debug, Default, Clone)]
pub struct Module {
    /// J05: as posições das funções viram tabelas de linha (`--debug_info`).
    /// Sem isto, as posições que houver são só do rastro simbólico (§13.14).
    pub dwarf: bool,
    /// As exceções do módulo são por tabelas (`--excecoes=tables`): o passe
    /// `optimize::tables` rodou e [`Module::tables`] vale. Falso: o modelo
    /// de sempre (pendência conferida depois de cada chamada).
    pub excecoes_por_tabelas: bool,
    /// A memória do módulo é ARC (`--memoria=arc`, `alvo::arc_memory`): a
    /// entrada liga o ARC no runtime e as gravações de referência em objeto
    /// que pode ser velho passam pelo runtime (docs/ARC-IMPLEMENTACAO.md).
    pub arc_memory: bool,
    /// Com [`Module::excecoes_por_tabelas`], a decisão do passe para cada
    /// função, na ordem de [`Module::functions`].
    pub tables: Vec<FunctionTables>,
    /// `dart:ffi`: (id RTI da classe, letra do tipo C) de cada tipo nativo
    /// e (chave da assinatura, símbolo do trampolim) (`lower/ffi.rs`).
    pub ffi_types: Vec<(i64, char)>,
    pub ffi_trampolines: Vec<(String, String)>,
    /// `dart:ffi`: as entradas C dos callbacks nativos, uma por assinatura
    /// (`lower/ffi.rs`; o emissor gera a entrada com a ABI C).
    pub ffi_callbacks: Vec<FfiCallback>,
    /// `dart:ffi`: as structs e unions do programa (`lower/ffi.rs`).
    pub ffi_composites: Vec<FfiComposite>,
    pub functions: Vec<Function>,
    /// Parâmetros escalares declarados na fonte, por símbolo/ID SSA.
    /// O lowering registra somente int/double/bool sem caixa. Parâmetros
    /// ocultos/nativos i64 não são classificados pela largura. Estes fatos
    /// ainda precisam alimentar os planos ARC e sobreviver aos passes.
    pub dart_scalar_parameters: std::collections::HashMap<String, std::collections::HashSet<ValueId>>,
    /// IDs nativos do universo canônico de RTI, produzidos explicitamente.
    /// Não são Ref nem escalares Dart; vida do universo/pins exige outro protocolo.
    pub dart_rti_parameters: std::collections::HashMap<String, std::collections::HashSet<ValueId>>,
    /// Vetores da ABI uniforme registrados explicitamente pelo lowering.
    /// Vida/bounds dependem do protocolo do chamador. Descritores são imutáveis;
    /// os primeiros dois slots são contagens nativas, sem referências do heap.
    pub dart_vector_parameters: std::collections::HashMap<String, std::collections::HashMap<ValueId, DartEntryVector>>,
    /// Corpos tipados de funções Dart da fonte cuja ABI devolve Ref Owned.
    /// Proveniência nominal do lowering; não inclui automaticamente entradas
    /// uniformes, callbacks FFI ou funções HIR fornecidas por outro produtor.
    pub dart_ref_returns: std::collections::HashSet<String>,
    /// Armazenamentos de globais Dart cuja representação, pelo tipo Dart, é
    /// escalar sem caixa (`int`, `double`, `bool`): o conteúdo nunca é handle.
    pub dart_scalar_globals: std::collections::HashSet<String>,
    /// Entradas uniformes declaradas pelo lowering para a ABI ARC com retorno Owned.
    /// São premissas para preparar o corpo inteiro, não certificados de dispatch
    /// indireto ou de callbacks C; as fronteiras exigem verificação própria.
    pub arc_uniform_returns: std::collections::HashSet<String>,
    /// Portas de uma imagem AOT fixa, após verificar os corpos e dependências.
    /// Alterações posteriores invalidam o inventário; não certifica recarga JIT.
    pub arc_aot_ports: Option<crate::optimize::arc::ArcAotPorts>,
    /// A preparação ARC de ownership publicou estes corpos. Sem ela, uma imagem
    /// ARC segue o regime legado (chamadas diretas, fábricas sem owner do
    /// código); com ela, a emissão exige o certificado de portas AOT.
    pub arc_prepared: bool,
    /// A preparação ARC aceita chamadas diretas a funções Dart de outra
    /// imagem já preparada (o SDK da fonte preparado), com o resumo tirado
    /// da própria chamada: argumentos pela representação dos operandos,
    /// falha possível e resultado Ref Owned (a convenção das funções Dart).
    pub arc_external_callees: bool,
    /// Caches de IDs nativos RTI declarados pelo lowering, com flag implícita `$ok`.
    /// Todas as escritas e representações exigem conferência antes de produzir contratos.
    pub arc_rti_caches: std::collections::HashSet<String>,
    /// Definições compartilháveis declaradas nominalmente pelo produtor ARC.
    /// Não certifica equivalência das cópias nem dependências da emissão.
    pub arc_shared_functions: std::collections::HashSet<String>,
    /// Funções locais diretas declaradas pelo lowering, fechadas à ligação externa.
    /// A poda ainda confere chamadas, endereços materializados e entradas publicadas.
    pub arc_direct_local_functions: std::collections::HashSet<String>,
    /// Origem nominal pré-otimização das alocações reconhecidas do lowering ARC.
    /// IDs SSA indexam a versão atual; cópias do inliner conservam o sítio de
    /// origem. Contextos especializados e dependências de recarga ainda faltam.
    pub arc_sites: std::collections::HashMap<String, std::collections::HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>>,
    /// Representações declaradas dos campos por classe/posição, somente no ARC.
    /// Inclui herança, mixins e prefixo de enum; late com inicializador fica Ref.
    /// Não certifica o tipo do receiver, forma física do heap ou versão/pins de recarga.
    pub arc_field_layouts: std::collections::HashMap<u32, Vec<Type>>,
    /// Os layouts declarados das classes do SDK da fonte preparado (outra
    /// imagem), na mesma forma de `arc_field_layouts`: só para conferir
    /// gravações e leituras abertas do programa preparado sozinho
    /// (`arc_external_callees`); não declaram classes deste módulo.
    pub arc_external_layouts: std::collections::HashMap<u32, Vec<Type>>,
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
    /// estável do valor `dfg.<path>`); a bandeira de inicialização é
    /// `<símbolo>$ok`.
    pub globals: Vec<(u32, Type, String)>,
    /// Construtos que o lowering não sabe baixar (N1). Não vazio = o
    /// programa não compila; o emissor produz só a mensagem.
    pub errors: Vec<String>,
    // --- P5c (SDK da fonte, δ) ---
    /// O módulo é parte de um programa com o SDK compilado da fonte: o
    /// código compartilhado entre módulos (entradas de tear-off, constantes
    /// canônicas) sai em `comdat`, e símbolos de outros módulos são
    /// declarados.
    pub sdk_mode: bool,
    /// O módulo é uma biblioteca do SDK (sem `dartforge_entry` nem o
    /// despacho de `toString`, que são do programa).
    pub sdk_library: bool,
    /// Tabelas de métodos das classes deste módulo, registradas no runtime
    /// na partida (P5c): (id da classe, [(seletor, símbolo do adaptador)]).
    pub method_tables: Vec<(u32, String, Vec<(String, String)>)>,
    /// A função que devolve a tabela de métodos de cada classe concreta
    /// compilada (de qualquer módulo), pelo id: a alocação de um objeto da
    /// classe registra a tabela (`dartforge_object_new_t`).
    pub table_functions: std::collections::HashMap<u32, String>,
    /// Biblioteca do SDK: o nome da função que registra as classes dela
    /// (nomes, subtipos, tabelas de métodos) no runtime.
    pub record: Option<String>,
    /// Programa com o SDK da fonte: as funções de registro das bibliotecas
    /// do SDK, chamadas por `dartforge_entry` antes das do programa.
    pub sdk_records: Vec<String>,
    /// Os campos do `dart:async` que o rastro percorre para achar quem espera
    /// (§13.14): `(id da class, "Classe.field", posição)`, registrados na
    /// partida com o rastro simbólico.
    pub trace_fields: Vec<(u32, String, usize)>,
    /// As entradas de tear-off que o rastro reconhece nos ramos de stream
    /// (§13.14): `(símbolo da entry, espécie)`, a espécie 1 para o
    /// `_StreamIterator._onData` e 2 para o `_StreamController._add`.
    pub stack_trace_tearoffs: Vec<(String, i64)>,
    /// Os ids das classes do programa, `(id, library, class)`: escritos
    /// no IR (`; df.class …`) para que a geração seguinte de uma recarga do
    /// JIT dê o mesmo id à mesma classe (J03, `Context::with_previous_ids`).
    pub program_ids: Vec<(u32, String, String)>,
    /// O layout dos objetos de cada classe do programa, para a migração das
    /// instâncias vivas numa recarga do JIT (J03): `(id, campos antes dos
    /// declarados — os de enum —, campos)`, escrito no IR como
    /// `; df.fields …`.
    pub program_fields: Vec<(u32, usize, Vec<LayoutField>)>,
    /// Programa com o SDK da fonte: os ids de classe dos valores que o
    /// runtime representa (`Null`, `_Smi`, `_Mint`, `_Double`, `bool`,
    /// `_OneByteString`, `_TwoByteString`, `_GrowableList`, `_List`,
    /// `_ImmutableList`, `_Closure`, `_Record`), na ordem de
    /// `runtime/src/selectors.rs`.
    pub runtime_cids: Vec<i64>,
    /// Programa com o SDK da fonte: a versão do SDK Dart compilado (o
    /// arquivo `version` dele), que `Platform.version` informa.
    pub sdk_version: Option<String>,
    /// Membros do SDK da fonte recusados neste módulo: (símbolo, motivo).
    /// Cada um virou uma função que avisa em tempo de execução.
    pub rejected: Vec<(String, String)>,
    /// Funções Dart que o runtime chama pelo nome (`_dartforge*` da
    /// sobreposição): (nome, símbolo), registradas pelo registro do módulo.
    pub helpers: Vec<(String, String)>,
    // --- P6 (bibliotecas da fonte, `source.rs`) ---------------------------
    /// Diagnósticos das funções de bibliotecas do SDK compiladas da fonte,
    /// pelo símbolo da função que os produziu: só viram `errors` se a poda
    /// (`source::prune`) mantiver a função.
    pub source_errors: Vec<(String, Vec<String>)>,
    /// Funções da fonte que o runtime chama sem que o programa as referencie
    /// (raízes da poda, além das funções do programa).
    pub source_roots: Vec<String>,
    /// A função que o laço de eventos do runtime usa para chamar uma
    /// closure sem argumentos (`dartforge_laco_de_eventos`, depois do
    /// `main`). `None`: o programa não usa `dart:async` e não há laço.
    pub call_dart: Option<String>,
    /// RTI: a função que registra o universo de tipos antes do `main`
    /// (`lower::rti::register_universe`); `None` sem receitas.
    pub start_rti: Option<String>,
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
