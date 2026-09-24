// Runtime nativo: tipos em tempo de execução (RTI, docs/NATIVO-PLANO.md
// §7.7), no desenho do dart2js (`sdk/lib/_internal/js_runtime/lib/rti.dart`):
//
// * um **universo** de tipos canônicos por isolado: cada tipo é um id
//   (`i64`) de uma tabela com *hash-consing* — o mesmo tipo tem sempre o
//   mesmo id, então igualdade de tipos é igualdade de ids;
// * **receitas**: o compilador descreve um tipo por um texto curto (a
//   gramática está em `Leitor`) que o runtime lê uma vez (o getter do
//   global da receita guarda o id). Uma receita pode ter **variáveis**:
//   `P<i>`, o i-ésimo parâmetro de tipo da classe que declara o código
//   corrente, e `M<i>`, o i-ésimo argumento de tipo da função genérica
//   corrente. `dartforge_rti_avaliar` troca as variáveis pelos tipos do
//   ambiente (o tipo de `this` visto como aquela classe, e a tupla de
//   argumentos da função) — o `_Rti._eval` do dart2js;
// * **regras de supertipo** por classe (o `_Universe.findRule` do dart2js):
//   para cada classe `C<X…>`, os supertipos dela escritos em `P<i>`;
//   registradas pelo código gerado na entrada;
// * o **tipo de cada objeto** genérico mora nos metadados do slot do heap
//   (`Heap::metadados`, o `metadata_ptr` reservado do cabeçalho); um objeto
//   sem metadado tem o tipo cru da classe (argumentos `dynamic`).
//
// A subtipagem segue as regras da especificação (Dart 3, sem tipos legados;
// "Subtypes", o `_isSubtype` do dart2js), com cache por par.

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Tipo {
    Dinamico,
    Vazio,
    Nunca,
    Nulo,
    /// Classe (id RTI) e argumentos.
    Interface(i64, Vec<i64>),
    /// `T?` (normalizado: `T` não é anulável nem topo).
    Anulavel(i64),
    FutureOr(i64),
    /// `R Function<X…>(pos…, [opc…], {nomeados…})`: `n_obrig` dos
    /// posicionais são obrigatórios; os nomeados vêm ordenados pelo nome.
    Funcao {
        genericos: usize,
        ret: i64,
        pos: Vec<i64>,
        n_obrig: usize,
        nomeados: Vec<(String, i64, bool)>,
    },
    /// Variável ligada de um tipo de função genérico (índice de De Bruijn).
    Ligada(usize),
    Registro {
        pos: Vec<i64>,
        nomeados: Vec<(String, i64)>,
    },
    /// Variáveis de receita: parâmetro da classe e argumento da função.
    ParamClasse(usize),
    ParamFuncao(usize),
    /// Os argumentos de tipo de uma chamada genérica (o ambiente `M<i>`).
    Tupla(Vec<i64>),
}

/// As classes do SDK que o runtime precisa conhecer (registradas pelo
/// código gerado): `dartforge_rti_classe_do_runtime`.
#[derive(Default)]
struct ClassesDoRuntime {
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
struct Universo {
    tipos: Vec<Tipo>,
    indice: HashMap<Tipo, i64>,
    /// Supertipos de cada classe: (classe do supertipo, modelo em `P<i>`).
    regras: HashMap<i64, Vec<(i64, i64)>>,
    /// Nome e número de parâmetros de tipo de cada classe (id RTI).
    classes: HashMap<i64, (String, usize)>,
    /// Classe RTI de um id de classe do heap que não é o mesmo número (as
    /// classes de erro 1000–1012 do runtime).
    do_heap: HashMap<i64, i64>,
    rt: ClassesDoRuntime,
    cache_sub: HashMap<(i64, i64), bool>,
    cache_aval: HashMap<(i64, i64, i64, i64), i64>,
    receitas: HashMap<Vec<u16>, i64>,
}

thread_local! {
    static RTI: RefCell<Universo> = RefCell::new(Universo::novo());
}

const T_DINAMICO: i64 = 0;
const T_VAZIO: i64 = 1;
const T_NUNCA: i64 = 2;
const T_NULO: i64 = 3;

impl Universo {
    fn novo() -> Self {
        let mut u = Universo::default();
        for t in [Tipo::Dinamico, Tipo::Vazio, Tipo::Nunca, Tipo::Nulo] {
            u.internar(t);
        }
        u
    }

    fn internar(&mut self, t: Tipo) -> i64 {
        if let Some(&id) = self.indice.get(&t) {
            return id;
        }
        let id = self.tipos.len() as i64;
        self.tipos.push(t.clone());
        self.indice.insert(t, id);
        id
    }

    fn tipo(&self, id: i64) -> &Tipo {
        &self.tipos[usize::try_from(id).expect("tipo inválido")]
    }

    fn e_topo(&self, id: i64) -> bool {
        match self.tipo(id) {
            Tipo::Dinamico | Tipo::Vazio => true,
            Tipo::Anulavel(x) => self.e_object(*x),
            Tipo::FutureOr(x) => self.e_topo(*x),
            _ => false,
        }
    }

    fn e_object(&self, id: i64) -> bool {
        matches!(self.tipo(id), Tipo::Interface(c, a) if *c == self.rt.object && a.is_empty())
    }

    /// `T?` normalizado (especificação, "Type normalization").
    fn anulavel(&mut self, t: i64) -> i64 {
        match self.tipo(t).clone() {
            Tipo::Dinamico | Tipo::Vazio | Tipo::Nulo | Tipo::Anulavel(_) => t,
            Tipo::Nunca => T_NULO,
            Tipo::FutureOr(x) if self.e_anulavel(x) => t,
            _ => self.internar(Tipo::Anulavel(t)),
        }
    }

    fn e_anulavel(&self, t: i64) -> bool {
        match self.tipo(t) {
            Tipo::Dinamico | Tipo::Vazio | Tipo::Nulo | Tipo::Anulavel(_) => true,
            Tipo::FutureOr(x) => self.e_anulavel(*x),
            _ => false,
        }
    }

    /// `FutureOr<T>` normalizado.
    fn future_or(&mut self, t: i64) -> i64 {
        if self.e_topo(t) || self.e_object(t) {
            return t;
        }
        match self.tipo(t) {
            Tipo::Nunca => self.interface(self.rt.future, vec![T_NUNCA]),
            Tipo::Nulo => {
                let f = self.interface(self.rt.future, vec![T_NULO]);
                self.anulavel(f)
            }
            _ => self.internar(Tipo::FutureOr(t)),
        }
    }

    fn interface(&mut self, classe: i64, args: Vec<i64>) -> i64 {
        self.internar(Tipo::Interface(classe, args))
    }

    /// A classe RTI de um id de classe do heap.
    fn classe_do_heap(&self, id: i64) -> i64 {
        self.do_heap.get(&id).copied().unwrap_or(id)
    }

    /// Tipo cru de uma classe: argumentos `dynamic`.
    fn cru(&mut self, classe: i64) -> i64 {
        let n = self.classes.get(&classe).map_or(0, |c| c.1);
        self.interface(classe, vec![T_DINAMICO; n])
    }

    /// Troca as variáveis de receita pelos tipos do ambiente.
    fn substituir(&mut self, t: i64, classe: &[i64], funcao: &[i64]) -> i64 {
        match self.tipo(t).clone() {
            Tipo::ParamClasse(i) => classe.get(i).copied().unwrap_or(T_DINAMICO),
            Tipo::ParamFuncao(i) => funcao.get(i).copied().unwrap_or(T_DINAMICO),
            Tipo::Interface(c, args) => {
                if args.is_empty() {
                    return t;
                }
                let a = args.iter().map(|&x| self.substituir(x, classe, funcao)).collect();
                self.interface(c, a)
            }
            Tipo::Anulavel(x) => {
                let y = self.substituir(x, classe, funcao);
                self.anulavel(y)
            }
            Tipo::FutureOr(x) => {
                let y = self.substituir(x, classe, funcao);
                self.future_or(y)
            }
            Tipo::Funcao { genericos, ret, pos, n_obrig, nomeados } => {
                let ret = self.substituir(ret, classe, funcao);
                let pos = pos.iter().map(|&x| self.substituir(x, classe, funcao)).collect();
                let nomeados = nomeados
                    .iter()
                    .map(|(n, x, r)| (n.clone(), self.substituir(*x, classe, funcao), *r))
                    .collect();
                self.internar(Tipo::Funcao { genericos, ret, pos, n_obrig, nomeados })
            }
            Tipo::Registro { pos, nomeados } => {
                let pos = pos.iter().map(|&x| self.substituir(x, classe, funcao)).collect();
                let nomeados = nomeados
                    .iter()
                    .map(|(n, x)| (n.clone(), self.substituir(*x, classe, funcao)))
                    .collect();
                self.internar(Tipo::Registro { pos, nomeados })
            }
            Tipo::Tupla(args) => {
                let a = args.iter().map(|&x| self.substituir(x, classe, funcao)).collect();
                self.internar(Tipo::Tupla(a))
            }
            _ => t,
        }
    }

    /// Os argumentos do supertipo `alvo` de `classe<args>` (a própria classe
    /// ou uma das regras dela).
    fn como_supertipo(&mut self, classe: i64, args: &[i64], alvo: i64) -> Option<Vec<i64>> {
        if classe == alvo {
            return Some(args.to_vec());
        }
        let modelo = self.regras.get(&classe)?.iter().find(|(c, _)| *c == alvo)?.1;
        let s = self.substituir(modelo, args, &[]);
        match self.tipo(s) {
            Tipo::Interface(_, a) => Some(a.clone()),
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
        let r = self.sub_sem_cache(s, t);
        self.cache_sub.insert((s, t), r);
        r
    }

    fn sub_sem_cache(&mut self, s: i64, t: i64) -> bool {
        if self.e_topo(t) {
            return true;
        }
        let ts = self.tipo(s).clone();
        let tt = self.tipo(t).clone();
        if matches!(ts, Tipo::Nunca) {
            return true;
        }
        if matches!(ts, Tipo::Dinamico | Tipo::Vazio) {
            return false;
        }
        if self.e_object(t) {
            return match ts {
                Tipo::Nulo | Tipo::Anulavel(_) => false,
                Tipo::FutureOr(x) => self.sub(x, t),
                _ => true,
            };
        }
        if matches!(ts, Tipo::Nulo) {
            return match tt {
                Tipo::Nulo | Tipo::Anulavel(_) => true,
                Tipo::FutureOr(y) => self.sub(T_NULO, y),
                _ => false,
            };
        }
        if let Tipo::Anulavel(x) = ts {
            return self.sub(T_NULO, t) && self.sub(x, t);
        }
        if let Tipo::FutureOr(x) = ts {
            let fx = self.interface(self.rt.future, vec![x]);
            return self.sub(fx, t) && self.sub(x, t);
        }
        if let Tipo::Anulavel(y) = tt {
            return self.sub(s, y);
        }
        if let Tipo::FutureOr(y) = tt {
            if self.sub(s, y) {
                return true;
            }
            let fy = self.interface(self.rt.future, vec![y]);
            return self.sub(s, fy);
        }
        match (ts, tt) {
            (Tipo::Interface(c1, a1), Tipo::Interface(c2, a2)) => {
                let Some(visto) = self.como_supertipo(c1, &a1, c2) else {
                    return false;
                };
                visto.iter().zip(a2.iter()).all(|(&x, &y)| self.sub(x, y))
            }
            (Tipo::Funcao { .. }, Tipo::Interface(c2, _)) => c2 == self.rt.function,
            (Tipo::Registro { .. }, Tipo::Interface(c2, _)) => c2 == self.rt.record,
            (
                Tipo::Funcao { genericos: g1, ret: r1, pos: p1, n_obrig: o1, nomeados: n1 },
                Tipo::Funcao { genericos: g2, ret: r2, pos: p2, n_obrig: o2, nomeados: n2 },
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
                for (nome, q, req) in &n2 {
                    match n1.iter().find(|(m, _, _)| m == nome) {
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
            (Tipo::Registro { pos: p1, nomeados: n1 }, Tipo::Registro { pos: p2, nomeados: n2 }) => {
                p1.len() == p2.len()
                    && n1.len() == n2.len()
                    && p1.iter().zip(&p2).all(|(&x, &y)| self.sub(x, y))
                    && n1.iter().zip(&n2).all(|((a, x), (b, y))| a == b && self.sub(*x, *y))
            }
            (Tipo::Ligada(i), Tipo::Ligada(j)) => i == j,
            _ => false,
        }
    }

    /// O texto de um tipo, como o `Type.toString()` da VM.
    fn texto(&self, t: i64) -> String {
        match self.tipo(t) {
            Tipo::Dinamico => "dynamic".to_string(),
            Tipo::Vazio => "void".to_string(),
            Tipo::Nunca => "Never".to_string(),
            Tipo::Nulo => "Null".to_string(),
            Tipo::Interface(c, args) => {
                let nome = self.classes.get(c).map_or_else(|| format!("Classe{c}"), |x| x.0.clone());
                if args.is_empty() {
                    nome
                } else {
                    let a: Vec<String> = args.iter().map(|&x| self.texto(x)).collect();
                    format!("{nome}<{}>", a.join(", "))
                }
            }
            Tipo::Anulavel(x) => format!("{}?", self.texto(*x)),
            Tipo::FutureOr(x) => format!("FutureOr<{}>", self.texto(*x)),
            Tipo::Funcao { ret, pos, n_obrig, nomeados, genericos } => {
                let mut partes: Vec<String> = pos[..*n_obrig].iter().map(|&x| self.texto(x)).collect();
                if pos.len() > *n_obrig {
                    let o: Vec<String> = pos[*n_obrig..].iter().map(|&x| self.texto(x)).collect();
                    partes.push(format!("[{}]", o.join(", ")));
                }
                if !nomeados.is_empty() {
                    let n: Vec<String> = nomeados
                        .iter()
                        .map(|(m, x, r)| format!("{}{} {m}", if *r { "required " } else { "" }, self.texto(*x)))
                        .collect();
                    partes.push(format!("{{{}}}", n.join(", ")));
                }
                let g = if *genericos > 0 {
                    let v: Vec<String> = (0..*genericos).map(|i| format!("X{i}")).collect();
                    format!("<{}>", v.join(", "))
                } else {
                    String::new()
                };
                format!("({}) => {}{g}", partes.join(", "), self.texto(*ret))
            }
            Tipo::Registro { pos, nomeados } => {
                let mut partes: Vec<String> = pos.iter().map(|&x| self.texto(x)).collect();
                if !nomeados.is_empty() {
                    let n: Vec<String> = nomeados.iter().map(|(m, x)| format!("{} {m}", self.texto(*x))).collect();
                    partes.push(format!("{{{}}}", n.join(", ")));
                }
                format!("({})", partes.join(", "))
            }
            Tipo::Ligada(i) => format!("X{i}"),
            Tipo::ParamClasse(i) => format!("P{i}"),
            Tipo::ParamFuncao(i) => format!("M{i}"),
            Tipo::Tupla(a) => {
                let v: Vec<String> = a.iter().map(|&x| self.texto(x)).collect();
                format!("<{}>", v.join(", "))
            }
        }
    }
}

/// Leitor de receitas (unidades UTF-16 do texto). A gramática:
///
/// ```text
/// T := 'D' | 'V' | 'N' | 'U'              dynamic, void, Never, Null
///    | 'C' n [ '<' T (',' T)* '>' ]       classe (id RTI) com argumentos
///    | 'O' '<' T '>'                      FutureOr
///    | 'P' n | 'M' n | 'B' n              parâmetro da classe, da função, variável ligada
///    | 'F' '<' g ';' T ';' n ';' T* ';' (nome ':' T ['!'])* '>'
///                                         função: genéricos, retorno, obrigatórios,
///                                         posicionais, nomeados ('!' = required)
///    | 'R' '<' T* ';' (nome ':' T)* '>'   record
///    | 'L' '<' T* '>'                     tupla de argumentos de tipo
///    | T '?'                              anulável
/// ```
/// Listas separadas por `,`.
struct Leitor<'x> {
    u: &'x [u16],
    i: usize,
}

impl Leitor<'_> {
    fn olhar(&self) -> Option<char> {
        self.u.get(self.i).map(|&c| char::from_u32(u32::from(c)).unwrap_or('\0'))
    }
    fn tomar(&mut self) -> char {
        let c = self.olhar().expect("receita de tipo truncada");
        self.i += 1;
        c
    }
    fn esperar(&mut self, c: char) {
        let d = self.tomar();
        assert!(d == c, "receita de tipo: esperava {c:?}, veio {d:?}");
    }
    fn numero(&mut self) -> i64 {
        let mut neg = false;
        if self.olhar() == Some('-') {
            self.i += 1;
            neg = true;
        }
        let mut n: i64 = 0;
        while let Some(c) = self.olhar().filter(char::is_ascii_digit) {
            n = n * 10 + i64::from(c as u8 - b'0');
            self.i += 1;
        }
        if neg { -n } else { n }
    }
    fn nome(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.olhar().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '$') {
            s.push(c);
            self.i += 1;
        }
        s
    }
    fn lista(&mut self, u: &mut Universo, fim: char) -> Vec<i64> {
        let mut v = Vec::new();
        if self.olhar() == Some(fim) {
            return v;
        }
        loop {
            v.push(self.tipo(u));
            if self.olhar() == Some(',') {
                self.i += 1;
            } else {
                return v;
            }
        }
    }
    fn tipo(&mut self, u: &mut Universo) -> i64 {
        let mut t = match self.tomar() {
            'D' => T_DINAMICO,
            'V' => T_VAZIO,
            'N' => T_NUNCA,
            'U' => T_NULO,
            'C' => {
                let c = self.numero();
                let mut args = Vec::new();
                if self.olhar() == Some('<') {
                    self.i += 1;
                    args = self.lista(u, '>');
                    self.esperar('>');
                }
                u.interface(c, args)
            }
            'O' => {
                self.esperar('<');
                let x = self.tipo(u);
                self.esperar('>');
                u.future_or(x)
            }
            'P' => {
                let n = self.numero() as usize;
                u.internar(Tipo::ParamClasse(n))
            }
            'M' => {
                let n = self.numero() as usize;
                u.internar(Tipo::ParamFuncao(n))
            }
            'B' => {
                let n = self.numero() as usize;
                u.internar(Tipo::Ligada(n))
            }
            'F' => {
                self.esperar('<');
                let genericos = self.numero() as usize;
                self.esperar(';');
                let ret = self.tipo(u);
                self.esperar(';');
                let n_obrig = self.numero() as usize;
                self.esperar(';');
                let pos = self.lista(u, ';');
                self.esperar(';');
                let mut nomeados = Vec::new();
                while self.olhar() != Some('>') {
                    let n = self.nome();
                    self.esperar(':');
                    let x = self.tipo(u);
                    let req = self.olhar() == Some('!');
                    if req {
                        self.i += 1;
                    }
                    nomeados.push((n, x, req));
                    if self.olhar() == Some(',') {
                        self.i += 1;
                    }
                }
                self.esperar('>');
                nomeados.sort_by(|a, b| a.0.cmp(&b.0));
                u.internar(Tipo::Funcao { genericos, ret, pos, n_obrig, nomeados })
            }
            'R' => {
                self.esperar('<');
                let pos = self.lista(u, ';');
                self.esperar(';');
                let mut nomeados = Vec::new();
                while self.olhar() != Some('>') {
                    let n = self.nome();
                    self.esperar(':');
                    let x = self.tipo(u);
                    nomeados.push((n, x));
                    if self.olhar() == Some(',') {
                        self.i += 1;
                    }
                }
                self.esperar('>');
                nomeados.sort_by(|a, b| a.0.cmp(&b.0));
                u.internar(Tipo::Registro { pos, nomeados })
            }
            'L' => {
                self.esperar('<');
                let a = self.lista(u, '>');
                self.esperar('>');
                u.internar(Tipo::Tupla(a))
            }
            c => panic!("receita de tipo: caractere inesperado {c:?}"),
        };
        while self.olhar() == Some('?') {
            self.i += 1;
            t = u.anulavel(t);
        }
        t
    }
}

use crate::heap::{smi, ValueTag};

/// O tipo de um valor (`Ref` ou escalar pela tag).
fn tipo_do_valor(u: &mut Universo, v: TaggedValue) -> i64 {
    match v.tag {
        ValueTag::Int => return u.interface(u.rt.int, Vec::new()),
        ValueTag::Double => return u.interface(u.rt.double, Vec::new()),
        ValueTag::Bool => return u.interface(u.rt.bool_, Vec::new()),
        ValueTag::Ref => {}
    }
    let h = v.bits;
    if h == 0 {
        return T_NULO;
    }
    if smi::e_smi(h) {
        return u.interface(u.rt.int, Vec::new());
    }
    enum Forma {
        Pronto(i64),
        Cru(i64),
        Registro(Vec<TaggedValue>),
    }
    let forma = HEAP.with(|heap| {
        let heap = heap.borrow();
        let meta = heap.metadado(h);
        if meta != 0 {
            return Forma::Pronto(meta - 1);
        }
        match heap.get(h) {
            Value::BoxedInt(_) => Forma::Cru(u.rt.int),
            Value::BoxedDouble(_) => Forma::Cru(u.rt.double),
            Value::BoxedBool(_) => Forma::Cru(u.rt.bool_),
            Value::String(_) => Forma::Cru(u.rt.string),
            Value::StringBuffer(_) => Forma::Cru(u.rt.string_buffer),
            Value::RegExp(_) => Forma::Cru(u.rt.regexp),
            Value::Match(_) => Forma::Cru(u.rt.matchc),
            Value::List(_) => Forma::Cru(u.rt.list),
            Value::Map(_) => Forma::Cru(u.rt.map),
            Value::Set(_) => Forma::Cru(u.rt.set),
            Value::Closure { .. } => Forma::Cru(u.rt.function),
            Value::Record(campos) => Forma::Registro(campos.clone()),
            Value::Object { class_id, .. } => Forma::Cru(u.classe_do_heap(*class_id)),
            Value::Cell(_) | Value::Environment(_) => Forma::Cru(u.rt.object),
        }
    });
    match forma {
        Forma::Pronto(t) => t,
        Forma::Cru(c) => u.cru(c),
        Forma::Registro(campos) => {
            let pos = campos.into_iter().map(|c| tipo_do_valor(u, c)).collect();
            u.internar(Tipo::Registro { pos, nomeados: Vec::new() })
        }
    }
}

/// O tipo de um `Ref` (null, `Smi`, caixa ou objeto).
fn tipo_do_ref(u: &mut Universo, r: i64) -> i64 {
    tipo_do_valor(u, TaggedValue::reference(r))
}

/// Os natives `_List` e `_GrowableList` recebem a tupla dos argumentos de
/// tipo do construtor como último parâmetro. Converte `L<E>` no tipo do
/// objeto concreto (`_List<E>` ou `_GrowableList<E>`) que será guardado no
/// metadado do slot do heap. A classe concreta preserva `P0` nos métodos do
/// SDK declarados nessas classes, como `_GrowableList.toList`.
pub(crate) fn tipo_lista_da_tupla(tupla: i64, classe_concreta: Option<i64>) -> Option<i64> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let classe = classe_concreta.unwrap_or(u.rt.list);
        if classe == 0 {
            return None;
        }
        let args = match u.tipo(tupla) {
            Tipo::Tupla(args) => args.clone(),
            _ => vec![T_DINAMICO],
        };
        Some(u.internar(Tipo::Interface(classe, args)))
    })
}

/// `_List._sliceInternal` cria outra classe concreta de lista, mas conserva
/// o argumento `E` do receptor. O native não recebe tupla de tipo separada.
pub(crate) fn tipo_lista_copiada(origem: i64, classe_concreta: Option<i64>) -> Option<i64> {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let classe = classe_concreta.unwrap_or(u.rt.list);
        if classe == 0 {
            return None;
        }
        let origem = tipo_do_ref(&mut u, origem);
        let args = match u.tipo(origem) {
            Tipo::Interface(_, args) => args.clone(),
            _ => vec![T_DINAMICO],
        };
        Some(u.internar(Tipo::Interface(classe, args)))
    })
}

// --- ABI do código gerado ------------------------------------------------

/// Registra uma classe do universo: id RTI, nome (`String` do heap) e
/// número de parâmetros de tipo.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_classe_nome(classe: i64, nome: i64, n_params: i64) {
    let nome = HEAP.with(|h| h.borrow().texto(nome).para_string());
    RTI.with(|u| {
        u.borrow_mut().classes.insert(classe, (nome, usize::try_from(n_params).unwrap_or(0)));
    });
}

/// Registra uma regra de supertipo: `classe` tem o supertipo `modelo` (uma
/// receita já lida, em `P<i>` dos parâmetros de `classe`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_regra(classe: i64, modelo: i64) {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let sup = match u.tipo(modelo) {
            Tipo::Interface(c, _) => *c,
            _ => return,
        };
        u.regras.entry(classe).or_default().push((sup, modelo));
    });
}

/// A classe RTI de uma das formas do runtime (0 int, 1 double, 2 bool,
/// 3 String, 4 List, 5 Map, 6 Set, 7 Function, 8 Record, 9 Future,
/// 10 Object, 11 StringBuffer, 12 RegExp, 13 Match, 14 Type) ou, com
/// `forma` ≥ 1000, a classe RTI do id de classe do heap `forma`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_classe_do_runtime(forma: i64, classe: i64) {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let rt = &mut u.rt;
        match forma {
            0 => rt.int = classe,
            1 => rt.double = classe,
            2 => rt.bool_ = classe,
            3 => rt.string = classe,
            4 => rt.list = classe,
            5 => rt.map = classe,
            6 => rt.set = classe,
            7 => rt.function = classe,
            8 => rt.record = classe,
            9 => rt.future = classe,
            10 => rt.object = classe,
            11 => rt.string_buffer = classe,
            12 => rt.regexp = classe,
            13 => rt.matchc = classe,
            14 => rt.type_ = classe,
            f => {
                u.do_heap.insert(f, classe);
            }
        }
    });
}

/// Lê uma receita (uma `String` do heap; o getter do global dela guarda o
/// resultado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_receita(texto: i64) -> i64 {
    let unidades = HEAP.with(|h| h.borrow().texto(texto).para_vec());
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        if let Some(&t) = u.receitas.get(&unidades) {
            return t;
        }
        let mut l = Leitor { u: &unidades, i: 0 };
        let t = l.tipo(&mut u);
        assert!(l.i == unidades.len(), "receita de tipo com sobra");
        u.receitas.insert(unidades, t);
        t
    })
}

/// A receita `modelo` no ambiente: `P<i>` é o i-ésimo argumento do tipo de
/// `this` visto como a classe `classe` (0 = sem `this`), `M<i>` o i-ésimo
/// da tupla `tupla` (0 = nenhuma: `dynamic`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_avaliar(modelo: i64, this: i64, classe: i64, tupla: i64) -> i64 {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let tipo_this = if this == 0 { 0 } else { tipo_do_ref(&mut u, this) };
        let chave = (modelo, tipo_this, classe, tupla);
        if let Some(&r) = u.cache_aval.get(&chave) {
            return r;
        }
        let args_classe: Vec<i64> = match u.tipo(tipo_this).clone() {
            Tipo::Interface(c, a) if this != 0 => u.como_supertipo(c, &a, classe).unwrap_or_default(),
            _ => Vec::new(),
        };
        let args_funcao: Vec<i64> = if tupla > 0 {
            match u.tipo(tupla) {
                Tipo::Tupla(a) => a.clone(),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let r = u.substituir(modelo, &args_classe, &args_funcao);
        u.cache_aval.insert(chave, r);
        r
    })
}

/// Grava o tipo de um objeto genérico (nos metadados do slot).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_definir(obj: i64, tipo: i64) {
    if !smi::e_handle(obj) {
        return;
    }
    // O lowering de um literal usa o tipo estático `List<E>`, mas o objeto
    // do heap é `_GrowableList<E>` (ou `_List<E>` quando fixo). Métodos do
    // SDK nessas classes avaliam `P0` sobre a classe concreta; conservar
    // apenas `List<E>` perderia E em chamadas como `lista.toList()`.
    let classe_lista = HEAP.with(|h| matches!(h.borrow().get(obj), Value::List(_)))
        .then(|| cid_do_runtime(obj))
        .flatten();
    let tipo = if let Some(classe) = classe_lista {
        RTI.with(|u| {
            let mut u = u.borrow_mut();
            match u.tipo(tipo) {
                Tipo::Interface(c, args) if *c == u.rt.list => {
                    let args = args.clone();
                    u.internar(Tipo::Interface(classe, args))
                }
                _ => tipo,
            }
        })
    } else {
        tipo
    };
    HEAP.with(|h| h.borrow_mut().set_metadado(obj, tipo + 1));
}

/// Grava o tipo estrutural de um record com campos nomeados. Os campos do
/// objeto já foram preenchidos em ordem canônica (posicionais, depois nomes
/// ordenados); os tipos vêm dos valores reais, como exige `Record.runtimeType`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_registro_nomeado(obj: i64, npos: i64, nomes: i64) {
    let npos = usize::try_from(npos).expect("número de campos posicionais inválido");
    let nomes = HEAP.with(|h| h.borrow().texto(nomes).para_string());
    let campos = HEAP.with(|h| {
        let h = h.borrow();
        let Value::Object { fields, .. } = h.get(obj) else { panic!("record nomeado esperado") };
        fields.clone()
    });
    let nomes: Vec<&str> = nomes.split(',').collect();
    assert_eq!(campos.len(), npos + nomes.len(), "forma do record nomeado");
    let tipo = RTI.with(|u| {
        let mut u = u.borrow_mut();
        let mut campos: Vec<i64> = campos.into_iter().map(|(bits, is_ref)| {
            assert!(is_ref, "campo de record sem referência");
            tipo_do_valor(&mut u, TaggedValue::reference(bits))
        }).collect();
        let nomeados = nomes.into_iter().zip(campos.drain(npos..)).map(|(n, t)| (n.to_string(), t)).collect();
        u.internar(Tipo::Registro { pos: campos, nomeados })
    });
    dartforge_rti_definir(obj, tipo);
}

/// O tipo de um valor `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_do_valor(v: i64) -> i64 {
    RTI.with(|u| tipo_do_ref(&mut u.borrow_mut(), v))
}

/// `v is T`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_e(v: i64, t: i64) -> u8 {
    RTI.with(|u| {
        let mut u = u.borrow_mut();
        let s = tipo_do_ref(&mut u, v);
        u8::from(u.sub(s, t))
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
    let falha = RTI.with(|u| {
        let mut u = u.borrow_mut();
        let s = tipo_do_ref(&mut u, v);
        if u.sub(s, t) {
            None
        } else {
            Some(format!("type '{}' is not a subtype of type '{}' in type cast", u.texto(s), u.texto(t)))
        }
    });
    if let Some(msg) = falha {
        let m = HEAP.with(|h| h.borrow_mut().allocate(Value::String(Texto::de_str(&msg))));
        let e = com_raizes(&[m], || dartforge_type_error_com_mensagem(m));
        com_raizes(&[e], || dartforge_exception_throw(e, 3));
    }
}

/// Id de classe do heap do objeto `Type` (o `_Type` da VM): um objeto com o
/// id do tipo no campo 0, canônico por tipo (`identical(A, A)`, chave de
/// mapa). O compilador registra a classe com o `toString` dela.
const CLASSE_TIPO: i64 = 0x3FFF_FF01;

thread_local! {
    static OBJETOS_TIPO: RefCell<HashMap<i64, i64>> = RefCell::new(HashMap::new());
}

/// O objeto `Type` canônico do tipo `t` (raiz permanente, como os valores
/// de enum).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_objeto_tipo(t: i64) -> i64 {
    if let Some(h) = OBJETOS_TIPO.with(|m| m.borrow().get(&t).copied()) {
        return h;
    }
    let h = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate(Value::Object { class_id: CLASSE_TIPO, fields: vec![(t, false)] });
        // Raiz permanente num id que os globais do programa (não negativos)
        // e o laço de eventos (negativos pequenos) não usam.
        heap.set_global_root(-(1_i64 << 40) - t, h);
        h
    });
    OBJETOS_TIPO.with(|m| m.borrow_mut().insert(t, h));
    h
}

/// O texto de um tipo (`Type.toString()`), como `String` do heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_texto(t: i64) -> i64 {
    let s = RTI.with(|u| u.borrow().texto(t));
    HEAP.with(|h| h.borrow_mut().allocate(Value::String(Texto::de_str(&s))))
}

/// `TypeError` com a mensagem (o layout do runtime: mensagem e rastro).
fn dartforge_type_error_com_mensagem(mensagem: i64) -> i64 {
    alocar_erro_com_rastro(1011, vec![(mensagem, true)])
}
