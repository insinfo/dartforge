// Runtime nativo: tipos em tempo de execução (RTI, docs/NATIVO-PLANO.md
// §7.7), no desenho do dart2js (`sdk/lib/_internal/js_runtime/lib/rti.dart`):
//
// * um **universo** de tipos canônicos por isolado: cada tipo é um id
//   (`i64`) de uma tabela com *hash-consing* — o mesmo tipo tem sempre o
//   mesmo id, então igualdade de tipos é igualdade de ids;
// * **receitas**: o compilador descreve um tipo por um texto curto (a
//   gramática está em `Reader`) que o runtime lê uma vez (o getter do
//   global da receita guarda o id). Uma receita pode ter **variáveis**:
//   `P<i>`, o i-ésimo parâmetro de tipo da classe que declara o código
//   corrente, e `M<i>`, o i-ésimo argumento de tipo da função genérica
//   corrente. `dartforge_rti_avaliar` troca as variáveis pelos tipos do
//   ambiente (o tipo de `this` visto como aquela classe, e a tupla de
//   argumentos da função) — o `_Rti._eval` do dart2js;
// * **regras de supertipo** por classe (o `_Universe.findRule` do dart2js):
//   para cada classe `C<X…>`, os supertipos dela escritos em `P<i>`;
//   registradas pelo código gerado na entrada;
// * o **tipo de cada objeto** genérico mora no `metadata` do cabeçalho do
//   bloco (`layout::Header`, `id + 1`; docs/NATIVO-ESPACO-UNIFICADO.md
//   §2.15), para todo objeto do espaço — listas e closures inclusive; um
//   objeto sem metadado tem o tipo cru da classe (argumentos `dynamic`).
//
// A subtipagem segue as regras da especificação (Dart 3, sem tipos legados;
// "Subtypes", o `_isSubtype` do dart2js), com cache por par.

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Type {
    Dynamic,
    Void,
    Never,
    Null,
    /// Classe (id RTI) e argumentos.
    Interface(i64, Vec<i64>),
    /// `T?` (normalizado: `T` não é anulável nem topo).
    Nullable(i64),
    FutureOr(i64),
    /// `R Function<X…>(pos…, [opc…], {named…})`: `n_required` dos
    /// posicionais são obrigatórios; os nomeados vêm ordenados pelo nome.
    Function {
        generics: usize,
        ret: i64,
        pos: Vec<i64>,
        n_required: usize,
        named: Vec<(String, i64, bool)>,
    },
    /// Variável ligada de um tipo de função genérico (índice de De Bruijn).
    Enabled(usize),
    Record {
        pos: Vec<i64>,
        named: Vec<(String, i64)>,
    },
    /// Variáveis de receita: parâmetro da classe e argumento da função.
    ClassParam(usize),
    FunctionParam(usize),
    /// Os argumentos de tipo de uma chamada genérica (o ambiente `M<i>`).
    Tuple(Vec<i64>),
}

/// As classes do SDK que o runtime precisa conhecer (registradas pelo
/// código gerado): `dartforge_rti_classe_do_runtime`.
#[derive(Default)]
struct RuntimeClasses {
    int: i64,
    double: i64,
    bool_: i64,
    string: i64,
    list: i64,
    map: i64,
    set: i64,
    function: i64,
    record: i64,
    future: i64,
    object: i64,
    string_buffer: i64,
    regexp: i64,
    matchc: i64,
    type_: i64,
}

#[derive(Default)]
struct Universe {
    /// Cópia local (sem trava) dos tipos do [`GlobalUniverse`] que este
    /// isolado já viu, pelo id.
    types: Vec<Option<&'static Type>>,
    ix: HashMap<Type, i64>,
    /// Supertipos de cada classe: (classe do supertipo, modelo em `P<i>`).
    rules: HashMap<i64, Vec<(i64, i64)>>,
    /// Nome e número de parâmetros de tipo de cada classe (id RTI).
    classes: HashMap<i64, (String, usize)>,
    /// Classe RTI de um id de classe do heap que não é o mesmo número (as
    /// classes de erro 1000–1012 do runtime).
    from_heap: HashMap<i64, i64>,
    rt: RuntimeClasses,
    cache_sub: HashMap<(i64, i64), bool>,
    eval_cache: HashMap<(i64, i64, i64, i64), i64>,
    recipes: HashMap<Vec<u16>, i64>,
    /// `Interface(class, [])` já internado, por classe: o tipo de todo
    /// `int`, `double`, `String`… de uma checagem, sem montar e hashear o
    /// `Type` a cada vez (ids internados nunca mudam).
    no_args: HashMap<i64, i64>,
    /// O tipo cru de cada classe, com a aridade com que foi calculado (uma
    /// recarga que mude a aridade recalcula).
    raws: HashMap<i64, (usize, i64)>,
    /// Memória direta (sem colisão resolvida: a entrada nova ocupa a vaga)
    /// de `v is t`, pela chave do valor ([`value_key`]): `(chave, t,
    /// resposta)`. Evita montar o tipo do valor e o cache de subtipos a cada
    /// conferência de covariância (`K`/`V` do `[]=` de um mapa, `E` do `add`
    /// de uma lista), que a VM faz com um teste de classe em linha.
    memo_and: Vec<(i64, i64, bool)>,
    /// O mesmo para `dartforge_rti_avaliar`: `[chave de this, modelo,
    /// classe, tupla, resultado]`.
    memo_eval: Vec<[i64; 5]>,
    /// O mesmo para `dartforge_rti_definir` de uma coleção do runtime:
    /// `[class concreta, tipo request, tipo gravado]` — o `List<E>` de um
    /// literal visto como `_GrowableList<E>` passava por `as_supertype`
    /// (e um `Vec` novo) a cada lista criada (N17).
    memo_set: Vec<[i64; 3]>,
}

/// O tamanho das memórias diretas do [`Universe`] (potência de 2).
const MEMO_N: usize = 1 << 10;

/// A vaga de uma chave nas memórias diretas.
fn memo_slot(a: i64, b: i64, c: i64) -> usize {
    let h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (b as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (c as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    (h >> (64 - MEMO_N.trailing_zeros())) as usize
}

/// Uma chave que determina o tipo de `h` em [`value_type`] sem o
/// montar (§2.15): o tipo reificado gravado no objeto (o metadado, > 0), ou,
/// sem ele, `-16 - cid` — para todo valor: `null` e `Smi` pelos cids 1 e 2,
/// os demais pelo cid do cabeçalho. Um record posicional sem metadado não
/// tem chave (o tipo dele sai dos campos). `None` também para o que não é
/// valor válido.
fn value_key(h: i64) -> Option<i64> {
    use crate::layout::{DESLOCAMENTO_DO_HANDLE, cid};
    if h == 0 {
        return Some(-16 - i64::from(cid::NULL));
    }
    if smi::is_smi(h) {
        return Some(-16 - i64::from(cid::SMI));
    }
    if !crate::layout::is_object(h) {
        return None;
    }
    // O metadado e a classe no cabeçalho, sem o heap (o handle é o bloco + 2;
    // vale também para os estáticos da imagem).
    // SAFETY: o handle de um objeto vivo (o valor que o código gerado
    // passou) aponta para o cabeçalho do bloco dele.
    #[allow(unsafe_code)]
    let c = unsafe { *((h - DESLOCAMENTO_DO_HANDLE) as *const crate::layout::Header) };
    if c.metadata != 0 {
        return Some(i64::from(c.metadata));
    }
    (c.class_id != cid::RECORD).then(|| -16 - i64::from(c.class_id))
}

thread_local! {
    static RTI: RefCell<Universe> = RefCell::new(Universe::new());
}

/// Os ids de tipo são do PROCESSO, não do isolado (docs/NATIVO-PROJETOS-REAIS.md,
/// C11): um id guardado como palavra crua — o argumento de tipo `T` que uma
/// closure genérica capturou no contexto, a tupla de tipos de uma chamada —
/// atravessa `Isolate.spawn` copiado como número, e no isolado novo tinha de
/// denotar o mesmo tipo. Com um universo por isolado, o id do principal
/// apontava para outro tipo (ou para fora da tabela: o pânico
/// "index out of bounds" do `NotificationServiceWorker` do new_sali/backend).
/// A VM manda os argumentos de tipo como objetos. Aqui a tabela canônica é
/// uma só, só de acréscimo (cada tipo vazado para viver o processo todo); cada
/// isolado guarda a sua cópia local dos que já viu, e só o tipo novo passa
/// pela trava.
struct GlobalUniverse {
    types: Vec<&'static Type>,
    ix: HashMap<Type, i64>,
}

fn global_universe() -> &'static std::sync::RwLock<GlobalUniverse> {
    static G: std::sync::OnceLock<std::sync::RwLock<GlobalUniverse>> = std::sync::OnceLock::new();
    G.get_or_init(|| {
        let mut g = GlobalUniverse { types: Vec::new(), ix: HashMap::default() };
        // Os ids fixos: T_DINAMICO, T_VAZIO, T_NUNCA, T_NULO.
        for t in [Type::Dynamic, Type::Void, Type::Never, Type::Null] {
            let id = g.types.len() as i64;
            g.types.push(Box::leak(Box::new(t.clone())));
            g.ix.insert(t, id);
        }
        std::sync::RwLock::new(g)
    })
}

/// O tipo `id` da tabela do processo.
fn global_type(id: i64) -> Option<&'static Type> {
    let g = global_universe().read().unwrap_or_else(|e| e.into_inner());
    usize::try_from(id).ok().and_then(|i| g.types.get(i).copied())
}

/// Vagas da [`FAST_MEMO`] (potência de 2).
const FAST_MEMO_N: usize = 256;

thread_local! {
    /// A memória direta de `v is t` na frente do [`Universe`]
    /// (docs/NATIVO-PLANO.md §13.2, item 7): `[key do value, t, response]`,
    /// a mesma chave e as mesmas respostas de `memo_and`, mas num estático de
    /// thread sem destrutor nem `RefCell`. O acerto dispensa o acesso ao
    /// universo, o `borrow` e as consultas de `test_without_value` — o que pesa
    /// nas duas conferências de covariância de cada `map[k] = v`. Esquecida
    /// junto com as memórias do universo ([`Universe::forget_memos`]).
    static FAST_MEMO: std::cell::UnsafeCell<[[i64; 3]; FAST_MEMO_N]> =
        const { std::cell::UnsafeCell::new([[0, -1, 0]; FAST_MEMO_N]) };
}

/// A vaga de `(key, t)` na [`FAST_MEMO`].
#[inline(always)]
fn fast_slot(key: i64, t: i64) -> usize {
    let h = (key as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (t as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    (h >> (64 - FAST_MEMO_N.trailing_zeros())) as usize
}

/// `v is t` guardado na [`FAST_MEMO`] para a chave do valor. Nenhuma chave
/// vale 0 e nenhum tipo -1: a vaga vazia nunca casa.
#[inline(always)]
#[allow(unsafe_code)]
fn fast_memo_read(key: i64, t: i64) -> Option<u8> {
    // SAFETY: a memória é desta thread e nenhuma referência a ela sobrevive à
    // leitura.
    let e = FAST_MEMO.with(|m| unsafe { (*m.get())[fast_slot(key, t)] });
    (e[0] == key && e[1] == t).then_some(e[2] as u8)
}

#[allow(unsafe_code)]
fn fast_memo_store(key: i64, t: i64, r: bool) {
    // SAFETY: como em `fast_memo_read`.
    FAST_MEMO.with(|m| unsafe { (*m.get())[fast_slot(key, t)] = [key, t, i64::from(r)] });
}

#[allow(unsafe_code)]
fn fast_memo_forget() {
    // SAFETY: como em `fast_memo_read`.
    FAST_MEMO.with(|m| unsafe { *m.get() = [[0, -1, 0]; FAST_MEMO_N] });
}

impl Type {
    /// O tipo com cada tipo-filho trocado por `f(child)`.
    fn with_children(&self, mut f: impl FnMut(i64) -> i64) -> Type {
        match self {
            Type::Interface(c, args) => Type::Interface(*c, args.iter().map(|&x| f(x)).collect()),
            Type::Nullable(x) => Type::Nullable(f(*x)),
            Type::FutureOr(x) => Type::FutureOr(f(*x)),
            Type::Function { generics, ret, pos, n_required, named } => Type::Function {
                generics: *generics,
                ret: f(*ret),
                pos: pos.iter().map(|&x| f(x)).collect(),
                n_required: *n_required,
                named: named.iter().map(|(n, x, r)| (n.clone(), f(*x), *r)).collect(),
            },
            Type::Record { pos, named } => Type::Record {
                pos: pos.iter().map(|&x| f(x)).collect(),
                named: named.iter().map(|(n, x)| (n.clone(), f(*x))).collect(),
            },
            Type::Tuple(args) => Type::Tuple(args.iter().map(|&x| f(x)).collect()),
            Type::Dynamic | Type::Void | Type::Never | Type::Null | Type::Enabled(_) | Type::ClassParam(_) | Type::FunctionParam(_) => self.clone(),
        }
    }
}

/// Os tipos de uma mensagem para outro isolado: cada id do universo deste
/// isolado vira um índice de uma tabela própria, com os filhos antes dos
/// pais (os ids de tipo são por isolado; os de classe, do programa).
#[derive(Clone, Default)]
pub struct MessageTypes {
    types: Vec<Type>,
    ix: HashMap<i64, i64>,
}

impl MessageTypes {
    /// O índice (na tabela da mensagem) do tipo `id` deste isolado.
    pub fn export(&mut self, id: i64) -> i64 {
        if let Some(&i) = self.ix.get(&id) {
            return i;
        }
        let t = RTI.with(|u| u.borrow().tipo(id).clone());
        let local = t.with_children(|child| self.export(child));
        let i = self.types.len() as i64;
        self.types.push(local);
        self.ix.insert(id, i);
        i
    }

    /// Os ids, no universo deste isolado, de todos os tipos da tabela.
    pub fn import(&self) -> Vec<i64> {
        RTI.with(|u| {
            let mut u = u.borrow_mut();
            let mut ids: Vec<i64> = Vec::with_capacity(self.types.len());
            for t in &self.types {
                let t = t.with_children(|child| ids[child as usize]);
                ids.push(u.intern(t));
            }
            ids
        })
    }
}

const T_DYNAMIC: i64 = 0;
const T_VOID: i64 = 1;
const T_NEVER: i64 = 2;
const T_NULL: i64 = 3;

impl Universe {
    fn new() -> Self {
        let mut u = Universe::default();
        for t in [Type::Dynamic, Type::Void, Type::Never, Type::Null] {
            u.intern(t);
        }
        u
    }

    /// Esquece as memórias diretas (uma regra, classe ou forma nova do
    /// runtime pode mudar uma resposta guardada).
    fn forget_memos(&mut self) {
        fast_memo_forget();
        self.memo_and.clear();
        self.memo_eval.clear();
        self.memo_set.clear();
    }

    /// `v is t` guardado para a chave do valor.
    fn memo_and_read(&self, key: i64, t: i64) -> Option<bool> {
        let (c, tt, r) = *self.memo_and.get(memo_slot(key, t, 0))?;
        (c == key && tt == t).then_some(r)
    }

    fn memo_and_store(&mut self, key: i64, t: i64, r: bool) {
        if self.memo_and.is_empty() {
            // Uma vaga vazia nunca casa: nenhuma chave vale 0 (o metadado
            // é > 0, as espécies < 0).
            self.memo_and = vec![(0, 0, false); MEMO_N];
        }
        let i = memo_slot(key, t, 0);
        self.memo_and[i] = (key, t, r);
    }

    /// `dartforge_rti_avaliar` guardado.
    fn memo_eval_read(&self, k: [i64; 4]) -> Option<i64> {
        let e = self.memo_eval.get(memo_slot(k[0], k[1], k[2] ^ k[3].rotate_left(32)))?;
        (e[..4] == k).then_some(e[4])
    }

    fn memo_eval_store(&mut self, k: [i64; 4], r: i64) {
        if self.memo_eval.is_empty() {
            self.memo_eval = vec![[0, -1, 0, 0, 0]; MEMO_N];
        }
        let i = memo_slot(k[0], k[1], k[2] ^ k[3].rotate_left(32));
        self.memo_eval[i] = [k[0], k[1], k[2], k[3], r];
    }

    fn intern(&mut self, t: Type) -> i64 {
        if let Some(&id) = self.ix.get(&t) {
            return id;
        }
        let (id, r) = {
            let mut g = global_universe().write().unwrap_or_else(|e| e.into_inner());
            match g.ix.get(&t) {
                Some(&id) => (id, g.types[id as usize]),
                None => {
                    let id = g.types.len() as i64;
                    let r: &'static Type = Box::leak(Box::new(t.clone()));
                    g.types.push(r);
                    g.ix.insert(t.clone(), id);
                    (id, r)
                }
            }
        };
        let i = id as usize;
        if self.types.len() <= i {
            self.types.resize(i + 1, None);
        }
        self.types[i] = Some(r);
        self.ix.insert(t, id);
        id
    }

    /// O tipo `id`, se existe (a cópia local, senão a tabela do processo).
    fn get_value(&self, id: i64) -> Option<&Type> {
        match usize::try_from(id).ok().and_then(|i| self.types.get(i).copied().flatten()) {
            Some(t) => Some(t),
            None => global_type(id),
        }
    }

    fn tipo(&self, id: i64) -> &Type {
        self.get_value(id).expect("tipo inválido")
    }

    fn is_top(&self, id: i64) -> bool {
        match self.tipo(id) {
            Type::Dynamic | Type::Void => true,
            Type::Nullable(x) => self.e_object(*x),
            Type::FutureOr(x) => self.is_top(*x),
            _ => false,
        }
    }

    /// `v is t` quando a resposta não depende do tipo do valor: `t` é um
    /// tipo topo, ou `Object` e `v` não é null. O caso de todo `as E` com
    /// `E` = `Object?`/`dynamic` nas coleções do SDK; poupa montar o tipo
    /// do valor (o heap, o metadado, o universo) e o cache de subtipos.
    fn test_without_value(&self, v: i64, t: i64) -> Option<bool> {
        if self.is_top(t) {
            return Some(true);
        }
        if v != 0 && self.e_object(t) {
            return Some(true);
        }
        None
    }

    fn e_object(&self, id: i64) -> bool {
        matches!(self.tipo(id), Type::Interface(c, a) if *c == self.rt.object && a.is_empty())
    }

    /// `T?` normalizado (especificação, "Type normalization").
    fn nullable(&mut self, t: i64) -> i64 {
        match self.tipo(t).clone() {
            Type::Dynamic | Type::Void | Type::Null | Type::Nullable(_) => t,
            Type::Never => T_NULL,
            Type::FutureOr(x) if self.is_nullable(x) => t,
            _ => self.intern(Type::Nullable(t)),
        }
    }

    fn is_nullable(&self, t: i64) -> bool {
        match self.tipo(t) {
            Type::Dynamic | Type::Void | Type::Null | Type::Nullable(_) => true,
            Type::FutureOr(x) => self.is_nullable(*x),
            _ => false,
        }
    }

    /// `FutureOr<T>` normalizado.
    fn future_or(&mut self, t: i64) -> i64 {
        if self.is_top(t) || self.e_object(t) {
            return t;
        }
        match self.tipo(t) {
            Type::Never => self.interface(self.rt.future, vec![T_NEVER]),
            Type::Null => {
                let f = self.interface(self.rt.future, vec![T_NULL]);
                self.nullable(f)
            }
            _ => self.intern(Type::FutureOr(t)),
        }
    }

    fn interface(&mut self, class: i64, args: Vec<i64>) -> i64 {
        if args.is_empty() {
            if let Some(&id) = self.no_args.get(&class) {
                return id;
            }
            let id = self.intern(Type::Interface(class, args));
            self.no_args.insert(class, id);
            return id;
        }
        self.intern(Type::Interface(class, args))
    }

    /// A classe RTI de um id de classe do heap.
    fn heap_class(&self, id: i64) -> i64 {
        self.from_heap.get(&id).copied().unwrap_or(id)
    }

    /// Tipo cru de uma classe: argumentos `dynamic`.
    fn raw_value(&mut self, class: i64) -> i64 {
        let n = self.classes.get(&class).map_or(0, |c| c.1);
        if let Some(&(arity, id)) = self.raws.get(&class)
            && arity == n
        {
            return id;
        }
        let id = self.interface(class, vec![T_DYNAMIC; n]);
        self.raws.insert(class, (n, id));
        id
    }

    /// Troca as variáveis de receita pelos tipos do ambiente.
    fn substitute(&mut self, t: i64, class: &[i64], function: &[i64]) -> i64 {
        match self.tipo(t).clone() {
            Type::ClassParam(i) => class.get(i).copied().unwrap_or(T_DYNAMIC),
            Type::FunctionParam(i) => function.get(i).copied().unwrap_or(T_DYNAMIC),
            Type::Interface(c, args) => {
                if args.is_empty() {
                    return t;
                }
                let a = args.iter().map(|&x| self.substitute(x, class, function)).collect();
                self.interface(c, a)
            }
            Type::Nullable(x) => {
                let y = self.substitute(x, class, function);
                self.nullable(y)
            }
            Type::FutureOr(x) => {
                let y = self.substitute(x, class, function);
                self.future_or(y)
            }
            Type::Function { generics, ret, pos, n_required, named } => {
                let ret = self.substitute(ret, class, function);
                let pos = pos.iter().map(|&x| self.substitute(x, class, function)).collect();
                let named = named
                    .iter()
                    .map(|(n, x, r)| (n.clone(), self.substitute(*x, class, function), *r))
                    .collect();
                self.intern(Type::Function { generics, ret, pos, n_required, named })
            }
            Type::Record { pos, named } => {
                let pos = pos.iter().map(|&x| self.substitute(x, class, function)).collect();
                let named = named
                    .iter()
                    .map(|(n, x)| (n.clone(), self.substitute(*x, class, function)))
                    .collect();
                self.intern(Type::Record { pos, named })
            }
            Type::Tuple(args) => {
                let a = args.iter().map(|&x| self.substitute(x, class, function)).collect();
                self.intern(Type::Tuple(a))
            }
            _ => t,
        }
    }

    /// Os argumentos do supertipo `target` de `class<args>` (a própria classe
    /// ou uma das regras dela).
    fn as_supertype(&mut self, class: i64, args: &[i64], target: i64) -> Option<Vec<i64>> {
        if class == target {
            return Some(args.to_vec());
        }
        let model = self.rules.get(&class)?.iter().find(|(c, _)| *c == target)?.1;
        let s = self.substitute(model, args, &[]);
        match self.tipo(s) {
            Type::Interface(_, a) => Some(a.clone()),
            _ => None,
        }
    }

    fn sub(&mut self, s: i64, t: i64) -> bool {
        if s == t {
            return true;
        }
        if let Some(&r) = self.cache_sub.get(&(s, t)) {
            return r;
        }
        let r = self.sub_without_cache(s, t);
        self.cache_sub.insert((s, t), r);
        r
    }

    fn sub_without_cache(&mut self, s: i64, t: i64) -> bool {
        if self.is_top(t) {
            return true;
        }
        let ts = self.tipo(s).clone();
        let tt = self.tipo(t).clone();
        if matches!(ts, Type::Never) {
            return true;
        }
        if matches!(ts, Type::Dynamic | Type::Void) {
            return false;
        }
        if self.e_object(t) {
            return match ts {
                Type::Null | Type::Nullable(_) => false,
                Type::FutureOr(x) => self.sub(x, t),
                _ => true,
            };
        }
        if matches!(ts, Type::Null) {
            return match tt {
                Type::Null | Type::Nullable(_) => true,
                Type::FutureOr(y) => self.sub(T_NULL, y),
                _ => false,
            };
        }
        if let Type::Nullable(x) = ts {
            return self.sub(T_NULL, t) && self.sub(x, t);
        }
        if let Type::FutureOr(x) = ts {
            let fx = self.interface(self.rt.future, vec![x]);
            return self.sub(fx, t) && self.sub(x, t);
        }
        if let Type::Nullable(y) = tt {
            return self.sub(s, y);
        }
        if let Type::FutureOr(y) = tt {
            if self.sub(s, y) {
                return true;
            }
            let fy = self.interface(self.rt.future, vec![y]);
            return self.sub(s, fy);
        }
        match (ts, tt) {
            (Type::Interface(c1, a1), Type::Interface(c2, a2)) => {
                let Some(seen) = self.as_supertype(c1, &a1, c2) else {
                    return false;
                };
                seen.iter().zip(a2.iter()).all(|(&x, &y)| self.sub(x, y))
            }
            (Type::Function { .. }, Type::Interface(c2, _)) => c2 == self.rt.function,
            (Type::Record { .. }, Type::Interface(c2, _)) => c2 == self.rt.record,
            (
                Type::Function { generics: g1, ret: r1, pos: p1, n_required: o1, named: n1 },
                Type::Function { generics: g2, ret: r2, pos: p2, n_required: o2, named: n2 },
            ) => {
                if g1 != g2 || o1 > o2 || p1.len() < p2.len() {
                    return false;
                }
                if !self.sub(r1, r2) {
                    return false;
                }
                for (i, &q) in p2.iter().enumerate() {
                    if !self.sub(q, p1[i]) {
                        return false;
                    }
                }
                for (name, q, req) in &n2 {
                    match n1.iter().find(|(m, _, _)| m == name) {
                        Some((_, p, _)) => {
                            if !self.sub(*q, *p) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                    let _ = req;
                }
                // Todo nomeado obrigatório de S tem de ser passado por T.
                n1.iter()
                    .filter(|(_, _, r)| *r)
                    .all(|(m, _, _)| n2.iter().any(|(n, _, r)| n == m && *r))
            }
            (Type::Record { pos: p1, named: n1 }, Type::Record { pos: p2, named: n2 }) => {
                p1.len() == p2.len()
                    && n1.len() == n2.len()
                    && p1.iter().zip(&p2).all(|(&x, &y)| self.sub(x, y))
                    && n1.iter().zip(&n2).all(|((a, x), (b, y))| a == b && self.sub(*x, *y))
            }
            (Type::Enabled(i), Type::Enabled(j)) => i == j,
            _ => false,
        }
    }

    /// O texto de um tipo, como o `Type.toString()` da VM.
    fn text(&self, t: i64) -> String {
        match self.tipo(t) {
            Type::Dynamic => "dynamic".to_string(),
            Type::Void => "void".to_string(),
            Type::Never => "Never".to_string(),
            Type::Null => "Null".to_string(),
            Type::Interface(c, args) => {
                let name = self.classes.get(c).map_or_else(|| format!("Classe{c}"), |x| x.0.clone());
                if args.is_empty() {
                    name
                } else {
                    let a: Vec<String> = args.iter().map(|&x| self.text(x)).collect();
                    format!("{name}<{}>", a.join(", "))
                }
            }
            Type::Nullable(x) => format!("{}?", self.text(*x)),
            Type::FutureOr(x) => format!("FutureOr<{}>", self.text(*x)),
            Type::Function { ret, pos, n_required, named, generics } => {
                let mut part_list: Vec<String> = pos[..*n_required].iter().map(|&x| self.text(x)).collect();
                if pos.len() > *n_required {
                    let o: Vec<String> = pos[*n_required..].iter().map(|&x| self.text(x)).collect();
                    part_list.push(format!("[{}]", o.join(", ")));
                }
                if !named.is_empty() {
                    let n: Vec<String> = named
                        .iter()
                        .map(|(m, x, r)| format!("{}{} {m}", if *r { "required " } else { "" }, self.text(*x)))
                        .collect();
                    part_list.push(format!("{{{}}}", n.join(", ")));
                }
                let g = if *generics > 0 {
                    // Como a VM: `<Y0, Y1>(Y0) => Y1`.
                    let v: Vec<String> = (0..*generics).map(|i| format!("Y{i}")).collect();
                    format!("<{}>", v.join(", "))
                } else {
                    String::new()
                };
                format!("{g}({}) => {}", part_list.join(", "), self.text(*ret))
            }
            Type::Record { pos, named } => {
                let mut part_list: Vec<String> = pos.iter().map(|&x| self.text(x)).collect();
                if !named.is_empty() {
                    let n: Vec<String> = named.iter().map(|(m, x)| format!("{} {m}", self.text(*x))).collect();
                    part_list.push(format!("{{{}}}", n.join(", ")));
                }
                format!("({})", part_list.join(", "))
            }
            Type::Enabled(i) => format!("Y{i}"),
            Type::ClassParam(i) => format!("P{i}"),
            Type::FunctionParam(i) => format!("M{i}"),
            Type::Tuple(a) => {
                let v: Vec<String> = a.iter().map(|&x| self.text(x)).collect();
                format!("<{}>", v.join(", "))
            }
        }
    }
}

/// Leitor de receitas (unidades UTF-16 do texto). A gramática:
///
/// ```text
/// T := 'D' | 'V' | 'N' | 'U'              dynamic, void, Never, Null
///    | 'C' n [ '<' T (',' T)* '>' ]       class (id RTI) com arguments
///    | 'O' '<' T '>'                      FutureOr
///    | 'P' n | 'M' n | 'B' n              pairâmetro da class, da função, variável ligada
///    | 'F' '<' g ';' T ';' n ';' T* ';' (name ':' T ['!'])* '>'
///                                         função: genéricos, ret, obrigatórios,
///                                         positional, named ('!' = required)
///    | 'R' '<' T* ';' (name ':' T)* '>'   record
///    | 'L' '<' T* '>'                     tuple de arguments de tipo
///    | T '?'                              anulável
/// ```
/// Listas separadas por `,`.
struct Reader<'x> {
    u: &'x [u16],
    i: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<char> {
        self.u.get(self.i).map(|&c| char::from_u32(u32::from(c)).unwrap_or('\0'))
    }
    fn take(&mut self) -> char {
        let c = self.peek().expect("receita de tipo truncada");
        self.i += 1;
        c
    }
    fn wait_for(&mut self, c: char) {
        let d = self.take();
        assert!(d == c, "receita de tipo: esperava {c:?}, veio {d:?}");
    }
    fn number(&mut self) -> i64 {
        let mut neg = false;
        if self.peek() == Some('-') {
            self.i += 1;
            neg = true;
        }
        let mut n: i64 = 0;
        while let Some(c) = self.peek().filter(char::is_ascii_digit) {
            n = n * 10 + i64::from(c as u8 - b'0');
            self.i += 1;
        }
        if neg { -n } else { n }
    }
    fn name(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '$') {
            s.push(c);
            self.i += 1;
        }
        s
    }
    fn list(&mut self, u: &mut Universe, end: char) -> Vec<i64> {
        let mut v = Vec::new();
        if self.peek() == Some(end) {
            return v;
        }
        loop {
            v.push(self.tipo(u));
            if self.peek() == Some(',') {
                self.i += 1;
            } else {
                return v;
            }
        }
    }
    fn tipo(&mut self, u: &mut Universe) -> i64 {
        let mut t = match self.take() {
            'D' => T_DYNAMIC,
            'V' => T_VOID,
            'N' => T_NEVER,
            'U' => T_NULL,
            'C' => {
                let c = self.number();
                let mut args = Vec::new();
                if self.peek() == Some('<') {
                    self.i += 1;
                    args = self.list(u, '>');
                    self.wait_for('>');
                }
                u.interface(c, args)
            }
            'O' => {
                self.wait_for('<');
                let x = self.tipo(u);
                self.wait_for('>');
                u.future_or(x)
            }
            'P' => {
                let n = self.number() as usize;
                u.intern(Type::ClassParam(n))
            }
            'M' => {
                let n = self.number() as usize;
                u.intern(Type::FunctionParam(n))
            }
            'B' => {
                let n = self.number() as usize;
                u.intern(Type::Enabled(n))
            }
            'F' => {
                self.wait_for('<');
                let generics = self.number() as usize;
                self.wait_for(';');
                let ret = self.tipo(u);
                self.wait_for(';');
                let n_required = self.number() as usize;
                self.wait_for(';');
                let pos = self.list(u, ';');
                self.wait_for(';');
                let mut named = Vec::new();
                while self.peek() != Some('>') {
                    let n = self.name();
                    self.wait_for(':');
                    let x = self.tipo(u);
                    let req = self.peek() == Some('!');
                    if req {
                        self.i += 1;
                    }
                    named.push((n, x, req));
                    if self.peek() == Some(',') {
                        self.i += 1;
                    }
                }
                self.wait_for('>');
                named.sort_by(|a, b| a.0.cmp(&b.0));
                u.intern(Type::Function { generics, ret, pos, n_required, named })
            }
            'R' => {
                self.wait_for('<');
                let pos = self.list(u, ';');
                self.wait_for(';');
                let mut named = Vec::new();
                while self.peek() != Some('>') {
                    let n = self.name();
                    self.wait_for(':');
                    let x = self.tipo(u);
                    named.push((n, x));
                    if self.peek() == Some(',') {
                        self.i += 1;
                    }
                }
                self.wait_for('>');
                named.sort_by(|a, b| a.0.cmp(&b.0));
                u.intern(Type::Record { pos, named })
            }
            'L' => {
                self.wait_for('<');
                let a = self.list(u, '>');
                self.wait_for('>');
                u.intern(Type::Tuple(a))
            }
            c => panic!("receita de tipo: caractere inesperado {c:?}"),
        };
        while self.peek() == Some('?') {
            self.i += 1;
            t = u.nullable(t);
        }
        t
    }
}

// `smi` para todos os fragmentos.
use crate::heap::smi;

/// O tipo de um valor sem caixa ou numa posição `Ref`.
#[allow(dead_code)]
fn value_type(u: &mut Universe, v: crate::heap::Value) -> i64 {
    use crate::heap::Value;
    match v {
        Value::Int(_) => u.interface(u.rt.int, Vec::new()),
        Value::Double(_) => u.interface(u.rt.double, Vec::new()),
        Value::Bool(_) => u.interface(u.rt.bool_, Vec::new()),
        Value::Ref(r) => ref_type(u, r),
    }
}

/// O tipo de um `Ref` (null, `Smi` ou objeto), despachado pelo cid (§2.15).
fn ref_type(u: &mut Universe, h: i64) -> i64 {
    use crate::layout::cid;
    if h == 0 {
        return T_NULL;
    }
    if smi::is_smi(h) {
        return u.interface(u.rt.int, Vec::new());
    }
    enum Shape {
        Ready(i64),
        Raw(i64),
        Record(Vec<i64>),
    }
    let shape = HEAP.with(|heap| {
        let heap = heap.borrow();
        let c = *heap.header(h);
        if c.metadata != 0 {
            return Shape::Ready(i64::from(c.metadata) - 1);
        }
        match c.class_id {
            cid::MINT => Shape::Raw(u.rt.int),
            cid::DOUBLE => Shape::Raw(u.rt.double),
            cid::BOOL => Shape::Raw(u.rt.bool_),
            cid::ONE_BYTE_STRING | cid::TWO_BYTE_STRING => Shape::Raw(u.rt.string),
            cid::CLOSURE => Shape::Raw(u.rt.function),
            cid::RECORD => Shape::Record(heap.record(h).map(<[i64]>::to_vec).unwrap_or_default()),
            // As classes internas do runtime (contexto, célula, acumulador,
            // programa de `RegExp`) nunca chegam ao programa como valor.
            cid::CONTEXTO | cid::CELULA | cid::STRING_ACCUMULATOR | cid::REGEXP_PROGRAM => Shape::Raw(u.rt.object),
            // Lista sem tipo gravado (literal de `E` dinâmico, lista do
            // runtime), lista tipada, objeto: a classe concreta
            // (`_GrowableList`, `_List`, `_ImmutableList`…), como na VM —
            // `List` cru não é subtipo de `ListBase`, e o `addAll` do SDK
            // converte para ele.
            c => Shape::Raw(u.heap_class(i64::from(c))),
        }
    });
    match shape {
        Shape::Ready(t) => t,
        Shape::Raw(c) => u.raw_value(c),
        Shape::Record(fields) => {
            let pos = fields.into_iter().map(|c| ref_type(u, c)).collect();
            u.intern(Type::Record { pos, named: Vec::new() })
        }
    }
}

/// Os natives `_List` e `_GrowableList` recebem a tupla dos argumentos de
/// tipo do construtor como último parâmetro. Converte `L<E>` no tipo do
/// objeto concreto (`_List<E>` ou `_GrowableList<E>`) que será guardado no
/// metadado do slot do heap. A classe concreta preserva `P0` nos métodos do
/// SDK declarados nessas classes, como `_GrowableList.toList`.
pub(crate) fn tuple_list_type(tuple: i64, concrete_class: Option<i64>) -> Option<i64> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let class = concrete_class.unwrap_or(u.rt.list);
        if class == 0 {
            return None;
        }
        let args = match u.tipo(tuple) {
            Type::Tuple(args) => args.clone(),
            _ => vec![T_DYNAMIC],
        };
        Some(u.intern(Type::Interface(class, args)))
    })
}

/// O tipo reificado `meta` de uma lista (`Interface(List, [E])`) aceita um
/// valor `int` (`code` 1), `double` (2) ou `bool` (3)? Só lê o universo
/// (sem internar): `E` é `dynamic`, `void`, `Object`, a própria classe, ou
/// um anulável ou `FutureOr` deles. Outro `E` (`num`, `Comparable`…)
/// responde falso, e quem pergunta segue pelo `[]=` do SDK, que confere.
pub(crate) fn list_accepts_scalar(meta: i64, code: i64) -> bool {
    RTI.with(|u| {
        let u = u.borrow();
        let target = match code {
            1 => u.rt.int,
            2 => u.rt.double,
            3 => u.rt.bool_,
            _ => return false,
        };
        let Some(Type::Interface(_, args)) = u.get_value(meta) else { return false };
        let mut e = args.first().copied().unwrap_or(T_DYNAMIC);
        loop {
            match u.get_value(e) {
                Some(Type::Dynamic | Type::Void) => return true,
                Some(Type::Nullable(x) | Type::FutureOr(x)) => e = *x,
                Some(Type::Interface(c, a)) => return a.is_empty() && (*c == target || *c == u.rt.object),
                _ => return false,
            }
        }
    })
}

/// A forma dos elementos (N14) de uma lista de tipo reificado `meta`
/// (`Interface(List, [E])`): compacta quando o `E` é exatamente `int`,
/// `double` ou `bool` não anuláveis — então só esse escalar entra pelo
/// `[]=`/`add` do SDK (a covariância confere o valor contra o `E`
/// reificado), e os 8 bytes dos bits bastam; senão a geral. Só lê o
/// universo.
pub(crate) fn type_list_shape(meta: i64) -> crate::listas::Element {
    use crate::listas::Element;
    RTI.with(|u| {
        let u = u.borrow();
        let Some(Type::Interface(_, args)) = u.get_value(meta) else { return Element::General };
        let Some(&e) = args.first() else { return Element::General };
        match u.get_value(e) {
            Some(Type::Interface(c, a)) if a.is_empty() && *c != 0 => {
                if *c == u.rt.int {
                    Element::Int
                } else if *c == u.rt.double {
                    Element::Double
                } else if *c == u.rt.bool_ {
                    Element::Bool
                } else {
                    Element::General
                }
            }
            _ => Element::General,
        }
    })
}

/// Põe a lista `h` na forma do tipo reificado dela (N14,
/// [`type_list_shape`]): chamado sempre que o metadado de uma lista
/// é gravado. A troca é do armazenamento (`Heap::list_adjust_shape`,
/// `listas.rs`, §2.15), que converte no lugar os elementos que já existem,
/// sem alocar; uma lista que ainda não cabe na forma compacta fica geral.
/// Nada disso é observável.
pub(crate) fn adjust_list_shape(heap: &mut Heap, h: i64) {
    if !crate::layout::is_object(h) || !heap.e_lista(h) {
        return;
    }
    let meta = heap.metadata(h);
    let shape = if meta == 0 { crate::listas::Element::General } else { type_list_shape(meta - 1) };
    if heap.list_shape(h) != shape {
        // A troca pode alocar (as caixas da forma geral, §2.16): a lista
        // fica enraizada enquanto isso (quem chama nem sempre a enraizou).
        let frame = heap.push_frame_with_slots(1);
        heap.set_root(frame, 0, h);
        heap.list_adjust_shape(h, shape);
        heap.pop_frame(frame);
    }
}

/// Grava o tipo reificado `meta` (o metadado, `tipo + 1`) da lista `h` e a
/// põe na forma dele ([`adjust_list_shape`]).
pub(crate) fn set_list_type(h: i64, meta: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        heap.set_metadata(h, meta);
        adjust_list_shape(&mut heap, h);
    });
}

/// `List<String>` na classe concreta de uma lista que o runtime cria (o
/// `Dart_NewListOfTypeFilled` da VM).
pub(crate) fn string_list_type(concrete_class: Option<i64>) -> Option<i64> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let class = concrete_class.unwrap_or(u.rt.list);
        if class == 0 || u.rt.string == 0 {
            return None;
        }
        let string = u.rt.string;
        let text = u.interface(string, Vec::new());
        Some(u.intern(Type::Interface(class, vec![text])))
    })
}

/// `_List._sliceInternal` cria outra classe concreta de lista, mas conserva
/// o argumento `E` do receptor. O native não recebe tupla de tipo separada.
pub(crate) fn copied_list_type(origin: i64, concrete_class: Option<i64>) -> Option<i64> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let class = concrete_class.unwrap_or(u.rt.list);
        if class == 0 {
            return None;
        }
        let origin = ref_type(&mut u, origin);
        let args = match u.tipo(origin) {
            Type::Interface(_, args) => args.clone(),
            _ => vec![T_DYNAMIC],
        };
        Some(u.intern(Type::Interface(class, args)))
    })
}

// --- ABI do código gerado ------------------------------------------------

/// Os registros da RTI do programa numa tabela (`lower/rti.rs`): uma linha
/// por registro — `C<id> <pairâmetros> <name>` (o nome da classe),
/// `R<id> <recipe>` (uma regra de supertipo) e `F<shape> <id>` (a classe de
/// uma forma do runtime) —, lidos na ordem. A tabela grande vem comprimida
/// (`tabela_rti`).
///
/// # Safety
/// `data` aponta para as palavras do vetor constante do módulo, com ao menos
/// `len` bytes legíveis (empacotados em little-endian): o texto UTF-8 ou a
/// forma comprimida dele. O vetor vive o processo inteiro.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_rti_iniciar_tabela(data: *const i64, len: i64) {
    // SAFETY: garantido por quem chama (uma constante do módulo).
    let bytes: &'static [u8] = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), usize::try_from(len).unwrap_or(0)) };
    let bytes = if bytes.starts_with(crate::tabela_rti::HEADER) { crate::tabela_rti::decompressed(bytes) } else { bytes };
    let text = std::str::from_utf8(bytes).expect("tabela da RTI em UTF-8");
    for line in text.lines() {
        let (tipo, rest) = line.split_at(1);
        let (a, b) = rest.split_once(' ').expect("registro da RTI");
        let id: i64 = a.parse().expect("id da RTI");
        match tipo {
            "C" => {
                let (n, name) = b.split_once(' ').unwrap_or((b, ""));
                let n: usize = n.parse().unwrap_or(0);
                RTI.with(|u| {
                    let mut u = u.borrow_mut();
                    u.classes.insert(id, (name.to_string(), n));
                    u.forget_memos();
                });
            }
            "R" => {
                let model = units_recipe(b.encode_utf16().collect());
                dartforge_rti_regra(id, model);
            }
            "F" => dartforge_rti_classe_do_runtime(id, b.parse().expect("id da RTI")),
            // A biblioteca e o nome declarado da classe (`dart:mirrors`).
            "B" => {
                let (uri, name) = b.split_once(' ').unwrap_or((b, ""));
                register_class_library(id, uri, name);
            }
            _ => panic!("registro da RTI desconhecido: {line}"),
        }
    }
}

/// Registra uma regra de supertipo: `class` tem o supertipo `model` (uma
/// receita já lida, em `P<i>` dos parâmetros de `class`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_regra(class: i64, model: i64) {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let sup = match u.tipo(model) {
            Type::Interface(c, _) => *c,
            _ => return,
        };
        // Sem duplicar: a publicação de uma recarga refaz as regras.
        let rules = u.rules.entry(class).or_default();
        if !rules.contains(&(sup, model)) {
            rules.push((sup, model));
            // Uma regra nova (a geração nova de uma recarga) pode mudar uma
            // resposta já guardada.
            u.cache_sub.clear();
            u.forget_memos();
        }
    });
}

/// A classe RTI de uma das formas do runtime (0 int, 1 double, 2 bool,
/// 3 String, 4 List, 5 Map, 6 Set, 7 Function, 8 Record, 9 Future,
/// 10 Object, 11 StringBuffer, 12 RegExp, 13 Match, 14 Type) ou, com
/// `shape` ≥ 1000, a classe RTI do id de classe do heap `shape`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_classe_do_runtime(shape: i64, class: i64) {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        u.forget_memos();
        let rt = &mut u.rt;
        match shape {
            0 => rt.int = class,
            1 => rt.double = class,
            2 => rt.bool_ = class,
            3 => rt.string = class,
            4 => rt.list = class,
            5 => rt.map = class,
            6 => rt.set = class,
            7 => rt.function = class,
            8 => rt.record = class,
            9 => rt.future = class,
            10 => rt.object = class,
            11 => rt.string_buffer = class,
            12 => rt.regexp = class,
            13 => rt.matchc = class,
            14 => rt.type_ = class,
            f => {
                u.from_heap.insert(f, class);
            }
        }
    });
}

/// Lê uma receita (uma `String` do heap; o getter do global dela guarda o
/// resultado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_receita(text: i64) -> i64 {
    let unidades: Vec<u16> = HEAP.with(|h| h.borrow().text(text).map(|t| t.unidades().collect()).unwrap_or_default());
    units_recipe(unidades)
}

/// A receita (em UTF-16) lida e internada.
fn units_recipe(unidades: Vec<u16>) -> i64 {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        if let Some(&t) = u.recipes.get(&unidades) {
            return t;
        }
        let mut l = Reader { u: &unidades, i: 0 };
        let t = l.tipo(&mut u);
        assert!(l.i == unidades.len(), "receita de tipo com sobra");
        u.recipes.insert(unidades, t);
        t
    })
}

/// A receita `model` no ambiente: `P<i>` é o i-ésimo argumento do tipo de
/// `this` visto como a classe `class` (0 = sem `this`), `M<i>` o i-ésimo
/// da tupla `tuple` (0 = nenhuma: `dynamic`).
///
/// O receiver é emprestado, inclusive os filhos de records sem metadado.
/// Só lê o heap; internamento e memórias usam estruturas nativas, sem coleta,
/// alocação Dart, escrita de campos, publicação de handles ou saída pending.
/// Os IDs são do universo RTI, não referências gerenciadas.
///
/// # Panics
/// IDs inconsistentes ou receiver sem vida válida: erro interno, sem exceção Dart.
///
/// ```
/// use dartforge_runtime::abi::dartforge_rti_avaliar;
/// assert_eq!(dartforge_rti_avaliar(0, 0, 0, 0), 0); // dynamic permanece dynamic.
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_avaliar(model: i64, this: i64, class: i64, tuple: i64) -> i64 {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        // O tipo de `this` só entra pela chave dele: a memória direta
        // responde sem montá-lo.
        let this_key = if this == 0 { Some(0) } else { value_key(this) };
        let memo = this_key.map(|c| [c, model, class, tuple]);
        if let Some(r) = memo.and_then(|k| u.memo_eval_read(k)) {
            return r;
        }
        let this_type = if this == 0 { 0 } else { ref_type(&mut u, this) };
        let key = (model, this_type, class, tuple);
        if let Some(&r) = u.eval_cache.get(&key) {
            if let Some(k) = memo {
                u.memo_eval_store(k, r);
            }
            return r;
        }
        let class_args: Vec<i64> = match u.tipo(this_type).clone() {
            Type::Interface(c, a) if this != 0 => u.as_supertype(c, &a, class).unwrap_or_default(),
            _ => Vec::new(),
        };
        let function_args: Vec<i64> = if tuple > 0 {
            match u.tipo(tuple) {
                Type::Tuple(a) => a.clone(),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let r = u.substitute(model, &class_args, &function_args);
        u.eval_cache.insert(key, r);
        if let Some(k) = memo {
            u.memo_eval_store(k, r);
        }
        r
    })
}

/// [`dartforge_rti_avaliar`] (sem tupla) de um ponto de uso com cache em
/// linha (`llvm/mod.rs`, N17): `cache` são as duas palavras do ponto na
/// área de globais do isolado — a chave de `this` ([`value_key`] de
/// um objeto do espaço: o metadado, ou `-16 - class`) e o resultado. O
/// código gerado compara a chave lida no cabeçalho de `this` com a
/// primeira palavra e só chama aqui na falha; aqui a resposta é gravada
/// quando `this` é objeto do espaço (os outros valores não têm a chave no
/// cabeçalho e voltam sempre ao runtime).
///
/// # Safety
/// `cache` aponta para duas palavras graváveis da área do isolado.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn dartforge_rti_avaliar_cache(cache: *mut i64, model: i64, this: i64, class: i64) -> i64 {
    let r = dartforge_rti_avaliar(model, this, class, 0);
    if crate::heap::is_object(this)
        && let Some(key) = value_key(this)
        && key != 0
    {
        // SAFETY: o contrato acima; só a thread do isolado grava a área.
        unsafe {
            *cache = key;
            *cache.add(1) = r;
        }
    }
    r
}

/// Grava o tipo de um objeto genérico (no `metadata` do cabeçalho).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_definir(obj: i64, tipo: i64) {
    if !crate::layout::is_object(obj) {
        return;
    }
    // O lowering dos literais reifica `List<E>`, `Map<K,V>` ou `Set<E>`, mas
    // os objetos pertencem às classes concretas do SDK. Os métodos dessas
    // classes (e de seus mixins) avaliam `P<i>` a partir do receptor.
    // A classe e o metadado no cabeçalho do bloco (`layout::Header`), sem
    // consultar o heap — como o código gerado lê.
    let header = (obj - crate::layout::DESLOCAMENTO_DO_HANDLE) as *mut crate::layout::Header;
    // SAFETY: o handle de um objeto vivo aponta para o cabeçalho dele.
    #[allow(unsafe_code)]
    let (estado, cid) = unsafe { ((*header).estado, (*header).class_id) };
    // Um estático da imagem (`PERMANENTE`) é só leitura e não tem tipo a
    // gravar (os literais de string e as caixas de `bool`).
    if estado == crate::layout::estado::PERMANENTE {
        return;
    }
    let concrete_class = Some(i64::from(cid));
    let tipo = if let Some(class) = concrete_class {
        RTI.with(|u| {
            let mut u = u.borrow_mut();
            // Só a interface de coleção de outra classe muda; o caso comum
            // (a classe do próprio objeto: `Future<T>`, `_Map<K, V>`…) sai
            // sem copiar o tipo. Só esse caso entra na memória, lida antes.
            let slot = memo_slot(class, tipo, 1);
            if let Some(&[mc, mt, r]) = u.memo_set.get(slot)
                && mc == class
                && mt == tipo
            {
                return r;
            }
            let (c, args) = match u.tipo(tipo) {
                Type::Interface(c, args) if *c != class && [u.rt.list, u.rt.map, u.rt.set].contains(c) => {
                    (*c, args.clone())
                }
                _ => return tipo,
            };
            let r = if u.as_supertype(class, &args, c).as_deref() == Some(args.as_slice()) {
                u.intern(Type::Interface(class, args))
            } else {
                tipo
            };
            if u.memo_set.is_empty() {
                // Vaga vazia nunca casa: nenhum tipo pedido vale -1.
                u.memo_set = vec![[0, -1, 0]; MEMO_N];
            }
            u.memo_set[slot] = [class, tipo, r];
            r
        })
    } else {
        tipo
    };
    // O metadado (`id + 1`) não é referência: sem barreira.
    // SAFETY: como acima; o cabeçalho é do objeto e só esta thread o grava.
    #[allow(unsafe_code)]
    unsafe {
        (*header).metadata = u32::try_from(tipo + 1).expect("metadado além de 32 bits");
    }
    // Um literal `<int>[…]` (N14): a lista compacta.
    if crate::layout::cid::e_lista(cid) {
        HEAP.with(|h| adjust_list_shape(&mut h.borrow_mut(), obj));
    }
}

/// Grava o tipo estrutural de um record com campos nomeados. Os campos do
/// objeto já foram preenchidos em ordem canônica (posicionais, depois nomes
/// ordenados); os tipos vêm dos valores reais, como exige `Record.runtimeType`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_registro_nomeado(obj: i64, npos: i64, names: i64) {
    let npos = usize::try_from(npos).expect("número de campos posicionais inválido");
    let names = HEAP.with(|h| h.borrow().text(names).map(|t| t.para_string()).unwrap_or_default());
    let fields = HEAP.with(|h| {
        let h = h.borrow();
        h.object(obj).expect("record nomeado esperado").to_vec()
    });
    let names: Vec<&str> = names.split(',').collect();
    assert_eq!(fields.len(), npos + names.len(), "forma do record nomeado");
    let tipo = RTI.with(|u| {
        let mut u = u.borrow_mut();
        let mut fields: Vec<i64> = fields.into_iter().map(|(bits, is_ref)| {
            assert!(is_ref, "campo de record sem referência");
            ref_type(&mut u, bits)
        }).collect();
        let named = names.into_iter().zip(fields.drain(npos..)).map(|(n, t)| (n.to_string(), t)).collect();
        u.intern(Type::Record { pos: fields, named })
    });
    dartforge_rti_definir(obj, tipo);
}

/// Texto com argumentos de tipo para o `toString` padrão de um valor
/// (`Instance of 'Caixa<int>'`, como a VM), ou `None` quando o tipo do valor
/// não tem argumentos — aí o chamador usa o nome registrado da classe.
pub(crate) fn text_with_arguments(v: i64) -> Option<String> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let t = ref_type(&mut u, v);
        let has_args = matches!(u.tipo(t), Type::Interface(_, args) if !args.is_empty());
        has_args.then(|| u.text(t))
    })
}

/// O tipo de um valor `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_do_valor(v: i64) -> i64 {
    RTI.with(|u| ref_type(&mut u.borrow_mut(), v))
}

/// `v is T`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_e(v: i64, t: i64) -> u8 {
    let key = value_key(v);
    if let Some(r) = key.and_then(|c| fast_memo_read(c, t)) {
        return r;
    }
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let r = if let Some(r) = u.test_without_value(v, t) {
            r
        } else if let Some(r) = key.and_then(|c| u.memo_and_read(c, t)) {
            r
        } else {
            let s = ref_type(&mut u, v);
            let r = u.sub(s, t);
            if debug() {
                eprintln!("[depurar] {} is {} = {r}", u.text(s), u.text(t));
            }
            if let Some(c) = key {
                u.memo_and_store(c, t, r);
            }
            r
        };
        if let Some(c) = key {
            fast_memo_store(c, t, r);
        }
        u8::from(r)
    })
}

/// `S <: T`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_subtipo(s: i64, t: i64) -> u8 {
    RTI.with(|u| u8::from(u.borrow_mut().sub(s, t)))
}

/// `v as T`: `TypeError` pendente se `v` não é um `T` (a mensagem da VM:
/// "type 'S' is not a subtype of type 'T' in type cast").
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_como(v: i64, t: i64) {
    dartforge_rti_como_em(v, t, 0, 0);
}

/// [`dartforge_rti_como`] com o contexto da conferência, que muda o fim da
/// mensagem da VM: 0 o `as` (" in type cast"), 1 a atribuição implícita de
/// um `dynamic` (sem sufixo), 2 o parâmetro `name` (um `String`) na entrada
/// de uma chamada dinâmica ou covariante (" of 'nome'").
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_como_em(v: i64, t: i64, contexto: i64, name: i64) {
    // O sufixo só é montado na falha: a conferência que passa (quase
    // todas) não aloca nada.
    let suffix = || match contexto {
        0 => " in type cast".to_string(),
        1 => String::new(),
        _ => format!(" of '{}'", HEAP.with(|h| h.borrow().text(name).map(|t| t.para_string()).unwrap_or_default())),
    };
    let failure = RTI.with(|u| {
        let mut u = u.borrow_mut();
        if u.test_without_value(v, t) == Some(true) {
            return None;
        }
        let key = value_key(v);
        if key.and_then(|c| u.memo_and_read(c, t)) == Some(true) {
            return None;
        }
        let s = ref_type(&mut u, v);
        let r = u.sub(s, t);
        if let Some(c) = key {
            u.memo_and_store(c, t, r);
        }
        if r {
            None
        } else {
            Some(format!("type '{}' is not a subtype of type '{}'{}", u.text(s), u.text(t), suffix()))
        }
    });
    if let Some(msg) = failure {
        let m = HEAP.with(|h| h.borrow_mut().allocate_str(&msg));
        let e = with_roots(&[m], || dartforge_type_error_com_mensagem(m));
        with_roots(&[e], || dartforge_exception_throw(e, 3));
    }
}

/// Id de classe do heap do objeto `Type` (o `_Type` da VM): um objeto com o
/// id do tipo no campo 0, canônico por tipo (`identical(A, A)`, chave de
/// mapa). O compilador registra a classe com o `toString` dela.
const TYPE_CLASS: i64 = 0x3FFF_FF01;

thread_local! {
    static TYPE_OBJECTS: RefCell<HashMap<i64, i64>> = RefCell::new(HashMap::default());
}

/// O objeto `Type` canônico do tipo `t` (raiz permanente, como os valores
/// de enum).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_objeto_tipo(t: i64) -> i64 {
    if let Some(h) = TYPE_OBJECTS.with(|m| m.borrow().get(&t).copied()) {
        return h;
    }
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.new_object(TYPE_CLASS, &[(t, false)]);
        // Raiz permanente num id que os globais do programa (não negativos)
        // e o laço de eventos (negativos pequenos) não usam. A sabotagem
        // `tipo_sem_raiz` (D12, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
        // §7.3) tira o registro: o cache passa a guardar um handle que a
        // coleta libera.
        if !crate::heap::sabotage("tipo_sem_raiz") {
            heap.set_global_root(-(1_i64 << 40) - t, h);
        }
        h
    });
    TYPE_OBJECTS.with(|m| m.borrow_mut().insert(t, h));
    h
}

/// [`dartforge_rti_objeto_tipo`] sob ARC: o objeto `Type` canônico com um
/// owner do código, independente da raiz permanente do cache. Pode alocar
/// na primeira consulta; não lança nem executa Dart.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let a = dartforge_arc_rti_objeto_tipo_owned_v1(7);
/// let b = dartforge_arc_rti_objeto_tipo_owned_v1(7);
/// assert_eq!(a, b);
/// assert_eq!(dartforge_arc_observar_heap_v1(a), 3);
/// dartforge_arc_release(a);
/// dartforge_arc_release(b);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_rti_objeto_tipo_owned_v1(t: i64) -> i64 {
    let h = dartforge_rti_objeto_tipo(t);
    HEAP.with(|heap| heap.borrow_mut().retain_code_owner(h));
    h
}

/// O texto de um tipo (`Type.toString()`), como `String` do heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_texto(t: i64) -> i64 {
    let s = RTI.with(|u| u.borrow().text(t));
    HEAP.with(|h| h.borrow_mut().allocate_str(&s))
}

/// O texto de um tipo sob ARC, com um owner do código do resultado.
///
/// `t` é um ID do universo RTI, não referência do heap. O chamador transfere
/// o owner ou chama release exatamente uma vez. Não executa Dart nem deixa
/// exceção pendente.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let text = dartforge_arc_rti_texto_owned_v1(0);
/// assert_eq!(dartforge_arc_observar_heap_v1(text), 3);
/// dartforge_arc_release(text);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(text), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_rti_texto_owned_v1(t: i64) -> i64 {
    let s = RTI.with(|u| u.borrow().text(t));
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let text = heap.allocate_str(&s);
        heap.retain_code_owner(text);
        text
    })
}

/// `TypeError` com a mensagem. No SDK da fonte, a exceção precisa ser a classe
/// `_TypeError` real para `e is TypeError` e o despacho de `toString`.
fn dartforge_type_error_com_mensagem(message: i64) -> i64 {
    if let Some(f) = ajudante("_dartforgeErroDeTipo") {
        // SAFETY: o helper registrado pelo `dart:core` recebe String e
        // devolve a instância concreta de `_TypeError`.
        let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
        return with_roots(&[message], || g(message));
    }
    allocate_error_with_trace(1011, vec![(message, true)])
}

#[cfg(test)]
mod tests_arc_borrowed_evaluation {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn evaluating_types_preserves_fields_owners_heap_and_pending() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(crate::heap::Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let previous_universe = RTI.with(|u| u.replace(Universe::new()));
            let recipe = |text: &str| units_recipe(text.encode_utf16().collect());
            let object = dartforge_arc_objeto_owned_v1(32001, 2);
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_gravar_campo_escalar_v1(object, 0, 42);
            dartforge_arc_gravar_campo_ref_v1(object, 1, child);
            dartforge_rti_definir(object, recipe("C32001<C200>"));
            let refs = [child, 0, crate::layout::smi::de(7).unwrap()];
            // SAFETY: vetor vivo de três Ref sustentados pelo chamador.
            let rec = unsafe { dartforge_record_novo(refs.as_ptr(), refs.len() as i64) };
            dartforge_arc_retain(rec);
            let parameter = recipe("P0");
            let method = recipe("M0");
            let tuple = recipe("L<C201>");
            let expected = recipe("C200");
            let expected_method = recipe("C201");
            let stats = HEAP.with(|h| h.borrow().stats());
            let pending = dartforge_exception_pending();
            for _ in 0..64 {
                assert_eq!(dartforge_rti_avaliar(parameter, object, 32001, 0), expected);
                assert_eq!(dartforge_rti_avaliar(method, 0, 0, tuple), expected_method);
                assert_eq!(dartforge_rti_avaliar(parameter, rec, 32001, 0), T_DYNAMIC);
                assert_eq!(dartforge_rti_avaliar(parameter, crate::layout::smi::de(7).unwrap(), 32001, 0), T_DYNAMIC);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                assert_eq!(h.stats().allocations, stats.allocations);
                assert_eq!(h.stats().collections, stats.collections);
                assert_eq!(h.object(object).unwrap().class_id, 32001);
                assert_eq!(h.object(object).unwrap().get(0), Some((42, false)));
                assert_eq!(h.object(object).unwrap().get(1), Some((child, true)));
            });
            assert_eq!(dartforge_exception_pending(), pending);
            assert_eq!(dartforge_arc_observar_heap_v1(child), 3);
            dartforge_arc_release(rec);
            dartforge_arc_release(object);
            dartforge_arc_release(child);
            HEAP.with(|h| h.replace(previous));
            RTI.with(|u| u.replace(previous_universe));
        }
    }
}
