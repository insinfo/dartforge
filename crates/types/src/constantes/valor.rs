//! Valores de constantes: o `DartObjectImpl` e os `InstanceState` do
//! analyzer 6.11 (`src/dart/constant/value.dart`), com as mesmas operações e
//! as mesmas exceções de avaliação (`EvaluationException`).
//!
//! Um valor tem o tipo em tempo de execução (com os tipos de extensão
//! apagados) e o estado. Estados com valor desconhecido (`UNKNOWN_VALUE`)
//! são válidos: a avaliação segue sem provar erro nenhum sobre eles.

use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::{codigos::compile_time_error as c, Codigo};
use dartforge_elements::model::{ClassId, FunctionElementId, VariableId};
use dartforge_intern::SymbolId;
use std::rc::Rc;

/// Texto de uma `String` do Dart: unidades UTF-16.
pub type Texto = Rc<[u16]>;

/// Chave de um campo de um objeto genérico: o nome do campo, ou o
/// pseudo-campo `(super)` com o objeto da superclasse (`GenericState.SUPERCLASS_FIELD`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Campo {
    Nome(SymbolId),
    Super,
    /// `index` e `_name` de uma constante de enum (`updateEnumConstant`).
    Indice,
    NomeDoEnum,
}

/// O elemento de um valor de função (`FunctionState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Funcao {
    Elemento(FunctionElementId),
    /// O construtor primário de um tipo de extensão (sem elemento).
    Primario(ClassId),
}

/// `InstanceState`.
#[derive(Clone, Debug)]
pub enum Estado {
    Bool(Option<bool>),
    Int(Option<i64>),
    Double(Option<f64>),
    Str(Option<Texto>),
    /// `NullState`; `invalido` é o objeto fictício de uma expressão não
    /// resolvida (`_unresolvedObject`).
    Null { invalido: bool },
    Simbolo(Option<Rc<str>>),
    Tipo(Option<TypeId>),
    Funcao { elemento: Funcao, args: Option<Rc<[TypeId]>> },
    Lista { elemento: TypeId, elementos: Rc<Vec<Valor>>, desconhecida: bool },
    Conjunto { elementos: Rc<Vec<Valor>>, desconhecido: bool },
    Mapa { entradas: Rc<Vec<(Valor, Valor)>>, desconhecido: bool },
    Registro { posicionais: Rc<Vec<Valor>>, nomeados: Rc<Vec<(SymbolId, Valor)>> },
    Generico { campos: Rc<Vec<(Campo, Valor)>>, desconhecido: bool },
}

/// `DartObjectImpl`.
#[derive(Clone, Debug)]
pub struct Valor {
    pub tipo: TypeId,
    pub estado: Estado,
    /// A variável constante a que o valor foi associado (`forVariable`).
    pub variavel: Option<VariableId>,
}

/// `EvaluationException`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Excecao {
    pub codigo: Codigo,
    pub de_execucao: bool,
}

impl Excecao {
    pub fn nova(codigo: Codigo) -> Self {
        Excecao { codigo, de_execucao: false }
    }
}

type R<T> = Result<T, Excecao>;

fn lanca<T>(codigo: Codigo) -> R<T> {
    Err(Excecao::nova(codigo))
}

impl Estado {
    pub fn desconhecido(&self) -> bool {
        match self {
            Estado::Bool(v) => v.is_none(),
            Estado::Int(v) => v.is_none(),
            Estado::Double(v) => v.is_none(),
            Estado::Str(v) => v.is_none(),
            Estado::Lista { desconhecida, .. } => *desconhecida,
            Estado::Conjunto { desconhecido, .. } | Estado::Mapa { desconhecido, .. } => *desconhecido,
            Estado::Generico { desconhecido, .. } => *desconhecido,
            _ => false,
        }
    }

    pub fn e_bool(&self) -> bool {
        matches!(self, Estado::Bool(_))
    }

    pub fn e_int(&self) -> bool {
        matches!(self, Estado::Int(_))
    }

    fn e_num(&self) -> bool {
        matches!(self, Estado::Int(_) | Estado::Double(_))
    }

    pub fn e_nulo(&self) -> bool {
        matches!(self, Estado::Null { .. })
    }

    pub fn e_bool_num_string_ou_nulo(&self) -> bool {
        matches!(self, Estado::Bool(_) | Estado::Int(_) | Estado::Double(_) | Estado::Str(_) | Estado::Null { .. })
    }

    fn afirmar_bool(s: &Estado) -> R<()> {
        if s.e_bool() { Ok(()) } else { lanca(c::CONST_EVAL_TYPE_BOOL) }
    }

    fn afirmar_int_ou_nulo(s: &Estado) -> R<()> {
        if matches!(s, Estado::Int(_) | Estado::Null { .. }) { Ok(()) } else { lanca(c::CONST_EVAL_TYPE_INT) }
    }

    fn afirmar_num_ou_nulo(s: &Estado) -> R<()> {
        if s.e_num() || s.e_nulo() { Ok(()) } else { lanca(c::CONST_EVAL_TYPE_NUM) }
    }

    fn afirmar_num_string_ou_nulo(s: &Estado) -> R<()> {
        if s.e_num() || matches!(s, Estado::Str(_)) || s.e_nulo() {
            Ok(())
        } else {
            lanca(c::CONST_EVAL_TYPE_NUM_STRING)
        }
    }

    fn afirmar_string(s: &Estado) -> R<()> {
        if matches!(s, Estado::Str(_)) { Ok(()) } else { lanca(c::CONST_EVAL_TYPE_STRING) }
    }

    /// `convertToBool`.
    pub fn para_bool(&self) -> R<Estado> {
        match self {
            Estado::Bool(v) => Ok(Estado::Bool(*v)),
            Estado::Null { .. } => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
            _ => Ok(Estado::Bool(Some(false))),
        }
    }

    /// `convertToString`.
    pub fn para_texto(&self, formatar_tipo: &dyn Fn(TypeId) -> String, nome_de_funcao: &dyn Fn(Funcao) -> String) -> R<Estado> {
        Ok(match self {
            Estado::Bool(v) => Estado::Str(v.map(|b| texto(if b { "true" } else { "false" }))),
            Estado::Int(v) => Estado::Str(v.map(|i| texto(&i.to_string()))),
            Estado::Double(v) => Estado::Str(v.map(|d| texto(&double_como_dart(d)))),
            Estado::Str(v) => Estado::Str(v.clone()),
            Estado::Null { .. } => Estado::Str(Some(texto("null"))),
            Estado::Simbolo(v) => Estado::Str(v.as_ref().map(|s| texto(s))),
            Estado::Tipo(t) => Estado::Str(t.map(|t| texto(&formatar_tipo(t)))),
            Estado::Funcao { elemento, .. } => Estado::Str(Some(texto(&nome_de_funcao(*elemento)))),
            Estado::Lista { .. } | Estado::Conjunto { .. } | Estado::Mapa { .. } | Estado::Registro { .. } | Estado::Generico { .. } => {
                Estado::Str(None)
            }
        })
    }

    /// `add`.
    pub fn somar(&self, d: &Estado) -> R<Estado> {
        match self {
            Estado::Int(v) => {
                Self::afirmar_num_ou_nulo(d)?;
                let Some(a) = v else {
                    return Ok(if matches!(d, Estado::Double(_)) { Estado::Double(None) } else { Estado::Int(None) });
                };
                match d {
                    Estado::Int(b) => Ok(Estado::Int(b.map(|b| a.wrapping_add(b)))),
                    Estado::Double(b) => Ok(Estado::Double(b.map(|b| *a as f64 + b))),
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            Estado::Double(v) => {
                Self::afirmar_num_ou_nulo(d)?;
                let Some(a) = v else { return Ok(Estado::Double(None)) };
                match d {
                    Estado::Int(b) => Ok(Estado::Double(b.map(|b| a + b as f64))),
                    Estado::Double(b) => Ok(Estado::Double(b.map(|b| a + b))),
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            Estado::Str(_) if matches!(d, Estado::Str(_)) => self.concatenar(d),
            _ => {
                Self::afirmar_num_string_ou_nulo(self)?;
                Self::afirmar_num_string_ou_nulo(d)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `concatenate`.
    pub fn concatenar(&self, d: &Estado) -> R<Estado> {
        if let Estado::Str(v) = self {
            let Some(a) = v else { return Ok(Estado::Str(None)) };
            if let Estado::Str(b) = d {
                let Some(b) = b else { return Ok(Estado::Str(None)) };
                let mut u: Vec<u16> = a.to_vec();
                u.extend_from_slice(b);
                return Ok(Estado::Str(Some(u.into())));
            }
        }
        Self::afirmar_string(d)?;
        lanca(c::CONST_EVAL_THROWS_EXCEPTION)
    }

    fn aritmetica(
        &self,
        d: &Estado,
        int: impl Fn(i64, i64) -> Option<R<Estado>>,
        double: impl Fn(f64, f64) -> R<Estado>,
        int_desconhecido_com_double: Estado,
    ) -> R<Estado> {
        match self {
            Estado::Int(v) => {
                Self::afirmar_num_ou_nulo(d)?;
                let Some(a) = *v else {
                    return Ok(if matches!(d, Estado::Double(_)) { int_desconhecido_com_double } else { Estado::Int(None) });
                };
                match d {
                    Estado::Int(b) => match b {
                        None => Ok(Estado::Int(None)),
                        Some(b) => int(a, *b).unwrap_or_else(|| lanca(c::CONST_EVAL_THROWS_EXCEPTION)),
                    },
                    Estado::Double(b) => match b {
                        None => Ok(Estado::Double(None)),
                        Some(b) => double(a as f64, *b),
                    },
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            Estado::Double(v) => {
                Self::afirmar_num_ou_nulo(d)?;
                let Some(a) = *v else { return Ok(Estado::Double(None)) };
                match d {
                    Estado::Int(b) => match b {
                        None => Ok(Estado::Double(None)),
                        Some(b) => double(a, *b as f64),
                    },
                    Estado::Double(b) => match b {
                        None => Ok(Estado::Double(None)),
                        Some(b) => double(a, *b),
                    },
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            _ => {
                Self::afirmar_num_ou_nulo(self)?;
                Self::afirmar_num_ou_nulo(d)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `minus`.
    pub fn subtrair(&self, d: &Estado) -> R<Estado> {
        self.aritmetica(d, |a, b| Some(Ok(Estado::Int(Some(a.wrapping_sub(b))))), |a, b| Ok(Estado::Double(Some(a - b))), Estado::Double(None))
    }

    /// `times`.
    pub fn multiplicar(&self, d: &Estado) -> R<Estado> {
        self.aritmetica(d, |a, b| Some(Ok(Estado::Int(Some(a.wrapping_mul(b))))), |a, b| Ok(Estado::Double(Some(a * b))), Estado::Double(None))
    }

    /// `divide`: sempre `double` (o `int` desconhecido também).
    pub fn dividir(&self, d: &Estado) -> R<Estado> {
        if let Estado::Int(None) = self {
            Self::afirmar_num_ou_nulo(d)?;
            return Ok(Estado::Double(None));
        }
        if let (Estado::Int(Some(_)), Estado::Int(None)) = (self, d) {
            return Ok(Estado::Double(None));
        }
        self.aritmetica(
            d,
            |a, b| Some(Ok(Estado::Double(Some(a as f64 / b as f64)))),
            |a, b| Ok(Estado::Double(Some(a / b))),
            Estado::Double(None),
        )
    }

    /// `remainder` (`%` do Dart: resto euclidiano, nunca negativo).
    pub fn resto(&self, d: &Estado) -> R<Estado> {
        self.aritmetica(
            d,
            |a, b| (b != 0).then(|| Ok(Estado::Int(Some(a.wrapping_rem_euclid(b))))),
            |a, b| Ok(Estado::Double(Some(resto_double(a, b)))),
            Estado::Double(None),
        )
    }

    /// `integerDivide` (`~/`).
    pub fn dividir_inteiro(&self, d: &Estado) -> R<Estado> {
        match self {
            Estado::Int(v) => {
                Self::afirmar_num_ou_nulo(d)?;
                let Some(a) = *v else { return Ok(Estado::Int(None)) };
                match d {
                    Estado::Int(None) | Estado::Double(None) => Ok(Estado::Int(None)),
                    Estado::Int(Some(0)) => Err(Excecao { codigo: c::CONST_EVAL_THROWS_IDBZE, de_execucao: true }),
                    Estado::Int(Some(b)) => Ok(Estado::Int(Some(a.wrapping_div(*b)))),
                    Estado::Double(Some(b)) => {
                        let r = a as f64 / b;
                        if r.is_finite() { Ok(Estado::Int(Some(r as i64))) } else { lanca(c::CONST_EVAL_THROWS_EXCEPTION) }
                    }
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            Estado::Double(v) => {
                Self::afirmar_num_ou_nulo(d)?;
                let Some(a) = *v else { return Ok(Estado::Int(None)) };
                let b = match d {
                    Estado::Int(None) | Estado::Double(None) => return Ok(Estado::Int(None)),
                    Estado::Int(Some(b)) => *b as f64,
                    Estado::Double(Some(b)) => *b,
                    _ => return lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                };
                let r = a / b;
                if r.is_finite() { Ok(Estado::Int(Some(r as i64))) } else { lanca(c::CONST_EVAL_THROWS_EXCEPTION) }
            }
            _ => {
                Self::afirmar_num_ou_nulo(self)?;
                Self::afirmar_num_ou_nulo(d)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    fn comparar(&self, d: &Estado, f: impl Fn(std::cmp::Ordering) -> bool) -> R<Estado> {
        let valor = |s: &Estado| -> Option<Option<f64>> {
            match s {
                Estado::Int(v) => Some(v.map(|i| i as f64)),
                Estado::Double(v) => Some(*v),
                _ => None,
            }
        };
        match (self, d) {
            (Estado::Int(Some(a)), Estado::Int(Some(b))) => Ok(Estado::Bool(Some(f(a.cmp(b))))),
            _ => {
                let Some(a) = valor(self) else {
                    Self::afirmar_num_ou_nulo(self)?;
                    Self::afirmar_num_ou_nulo(d)?;
                    return lanca(c::CONST_EVAL_THROWS_EXCEPTION);
                };
                Self::afirmar_num_ou_nulo(d)?;
                let Some(a) = a else { return Ok(Estado::Bool(None)) };
                let Some(b) = valor(d) else { return lanca(c::CONST_EVAL_THROWS_EXCEPTION) };
                let Some(b) = b else { return Ok(Estado::Bool(None)) };
                Ok(Estado::Bool(Some(match a.partial_cmp(&b) {
                    Some(o) => f(o),
                    None => false,
                })))
            }
        }
    }

    pub fn maior(&self, d: &Estado) -> R<Estado> {
        self.comparar(d, |o| o == std::cmp::Ordering::Greater)
    }
    pub fn maior_ou_igual(&self, d: &Estado) -> R<Estado> {
        self.comparar(d, |o| o != std::cmp::Ordering::Less)
    }
    pub fn menor(&self, d: &Estado) -> R<Estado> {
        self.comparar(d, |o| o == std::cmp::Ordering::Less)
    }
    pub fn menor_ou_igual(&self, d: &Estado) -> R<Estado> {
        self.comparar(d, |o| o != std::cmp::Ordering::Greater)
    }

    fn bits(&self, d: &Estado, f: impl Fn(i64, i64) -> Option<Estado>) -> R<Estado> {
        match self {
            Estado::Int(v) => {
                Self::afirmar_int_ou_nulo(d)?;
                let Some(a) = *v else { return Ok(Estado::Int(None)) };
                match d {
                    Estado::Int(None) => Ok(Estado::Int(None)),
                    Estado::Int(Some(b)) => f(a, *b).map_or_else(|| lanca(c::CONST_EVAL_THROWS_EXCEPTION), Ok),
                    _ => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
                }
            }
            _ => {
                Self::afirmar_int_ou_nulo(self)?;
                Self::afirmar_int_ou_nulo(d)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    pub fn e_bit(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| Some(Estado::Int(Some(a & b))))
    }
    pub fn ou_bit(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| Some(Estado::Int(Some(a | b))))
    }
    pub fn xor_bit(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| Some(Estado::Int(Some(a ^ b))))
    }
    /// `<<`: deslocamento com mais de 31 bits de largura é desconhecido.
    pub fn desloca_esquerda(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| {
            if bit_length(b) > 31 {
                Some(Estado::Int(None))
            } else if b >= 0 {
                Some(Estado::Int(Some(if b >= 64 { 0 } else { a.wrapping_shl(b as u32) })))
            } else {
                None
            }
        })
    }
    pub fn desloca_direita(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| {
            if bit_length(b) > 31 {
                Some(Estado::Int(None))
            } else if b >= 0 {
                Some(Estado::Int(Some(if b >= 64 { if a < 0 { -1 } else { 0 } } else { a >> b })))
            } else {
                None
            }
        })
    }
    pub fn desloca_direita_logico(&self, d: &Estado) -> R<Estado> {
        self.bits(d, |a, b| {
            if b >= 64 {
                Some(Estado::Int(Some(0)))
            } else if b >= 0 {
                Some(Estado::Int(Some(((a as u64) >> b) as i64)))
            } else {
                None
            }
        })
    }

    /// `bitNot`.
    pub fn negar_bits(&self) -> R<Estado> {
        match self {
            Estado::Int(v) => Ok(Estado::Int(v.map(|i| !i))),
            _ => {
                Self::afirmar_int_ou_nulo(self)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `negated`.
    pub fn negar(&self) -> R<Estado> {
        match self {
            Estado::Int(v) => Ok(Estado::Int(v.map(|i| i.wrapping_neg()))),
            Estado::Double(v) => Ok(Estado::Double(v.map(|d| -d))),
            _ => {
                Self::afirmar_num_ou_nulo(self)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }

    /// `logicalNot`.
    pub fn nao_logico(&self) -> R<Estado> {
        match self {
            Estado::Bool(v) => Ok(Estado::Bool(v.map(|b| !b))),
            Estado::Null { .. } => lanca(c::CONST_EVAL_THROWS_EXCEPTION),
            _ => {
                Self::afirmar_bool(self)?;
                Ok(Estado::Bool(Some(true)))
            }
        }
    }

    fn logico(&self, d: &Estado, f: impl Fn(bool, bool) -> bool) -> R<Estado> {
        Self::afirmar_bool(self)?;
        Self::afirmar_bool(d)?;
        let (Estado::Bool(a), Estado::Bool(b)) = (self.para_bool()?, d.para_bool()?) else { unreachable!() };
        Ok(Estado::Bool(match (a, b) {
            (Some(a), Some(b)) => Some(f(a, b)),
            _ => None,
        }))
    }
    pub fn e_logico(&self, d: &Estado) -> R<Estado> {
        self.logico(d, |a, b| a & b)
    }
    pub fn ou_logico(&self, d: &Estado) -> R<Estado> {
        self.logico(d, |a, b| a | b)
    }
    pub fn xor_logico(&self, d: &Estado) -> R<Estado> {
        self.logico(d, |a, b| a ^ b)
    }

    /// `stringLength` (em unidades UTF-16).
    pub fn comprimento(&self) -> R<Estado> {
        match self {
            Estado::Str(v) => Ok(Estado::Int(v.as_ref().map(|s| s.len() as i64))),
            _ => {
                Self::afirmar_string(self)?;
                lanca(c::CONST_EVAL_THROWS_EXCEPTION)
            }
        }
    }
}

/// `bitLength` do Dart: bits sem o sinal (`-1` tem 0).
fn bit_length(v: i64) -> u32 {
    let x = if v < 0 { !v } else { v };
    64 - x.leading_zeros()
}

fn resto_double(a: f64, b: f64) -> f64 {
    let r = a % b;
    if r < 0.0 { r + b.abs() } else { r }
}

/// O texto de uma `String` a partir de um `&str`.
pub fn texto(s: &str) -> Texto {
    s.encode_utf16().collect::<Vec<u16>>().into()
}

/// `double.toString()` do Dart: `1.0`, `0.1`, `1e+21`, `1e-7`, `NaN`,
/// `Infinity`.
pub fn double_como_dart(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if d == 0.0 {
        return if d.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let abs = d.abs();
    if (1e-6..1e21).contains(&abs) {
        let s = format!("{d}");
        if s.contains('.') { s } else { format!("{s}.0") }
    } else {
        let s = format!("{d:e}");
        match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        }
    }
}

impl Valor {
    /// O tipo já vem apagado (`extensionTypeErasure`, feito pelo motor).
    pub fn novo(tipo: TypeId, estado: Estado) -> Valor {
        Valor { tipo, estado, variavel: None }
    }

    pub fn nulo(core: &CoreTypes) -> Valor {
        Valor { tipo: core.null, estado: Estado::Null { invalido: false }, variavel: None }
    }

    pub fn bool_(core: &CoreTypes, v: Option<bool>) -> Valor {
        Valor { tipo: core.bool_, estado: Estado::Bool(v), variavel: None }
    }

    /// `validWithUnknownValue` (o tipo já apagado).
    pub fn desconhecido(table: &TypeTable, core: &CoreTypes, tipo: TypeId, elemento_de_lista: Option<TypeId>) -> Valor {
        let classe = match table.get(tipo) {
            Type::Interface { class, nullable: false, .. } => Some(*class),
            _ => None,
        };
        // O desconhecido de tipo anulável (`bool?`, `int?`) tem o estado do
        // tipo de base: pelo lado seguro, nenhum erro de tipo é provado sobre
        // ele (`c ? 1 : 2` com `c` desconhecido de tipo `bool?` não é
        // `CONST_EVAL_TYPE_BOOL`).
        let base = match table.get(tipo) {
            Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let de = |k: Option<ClassId>| base.is_some() && base == k;
        let estado = if tipo == core.bool_ || de(core.bool_class) {
            Estado::Bool(None)
        } else if tipo == core.double || de(core.double_class) {
            Estado::Double(None)
        } else if tipo == core.int || de(core.int_class) {
            Estado::Int(None)
        } else if tipo == core.string || de(core.string_class) {
            Estado::Str(None)
        } else if classe.is_some() && classe == core.list_class {
            Estado::Lista { elemento: elemento_de_lista.unwrap_or(core.dynamic_), elementos: Rc::new(Vec::new()), desconhecida: true }
        } else if classe.is_some() && classe == core.map_class {
            Estado::Mapa { entradas: Rc::new(Vec::new()), desconhecido: true }
        } else if classe.is_some() && classe == core.set_class {
            Estado::Conjunto { elementos: Rc::new(Vec::new()), desconhecido: true }
        } else {
            Estado::Generico { campos: Rc::new(Vec::new()), desconhecido: true }
        };
        Valor { tipo, estado, variavel: None }
    }

    pub fn desconhecido_de_fato(&self) -> bool {
        self.estado.desconhecido()
    }

    pub fn como_bool(&self) -> Option<bool> {
        match self.estado {
            Estado::Bool(v) => v,
            _ => None,
        }
    }

    pub fn como_texto(&self) -> Option<&Texto> {
        match &self.estado {
            Estado::Str(Some(s)) => Some(s),
            _ => None,
        }
    }

    pub fn como_tipo(&self) -> Option<TypeId> {
        match self.estado {
            Estado::Tipo(t) => t,
            _ => None,
        }
    }

    /// `toListValue`.
    pub fn como_lista(&self) -> Option<&Rc<Vec<Valor>>> {
        match &self.estado {
            Estado::Lista { elementos, .. } => Some(elementos),
            _ => None,
        }
    }

    /// `toSetValue`.
    pub fn como_conjunto(&self) -> Option<&Rc<Vec<Valor>>> {
        match &self.estado {
            Estado::Conjunto { elementos, .. } => Some(elementos),
            _ => None,
        }
    }

    /// `toMapValue`.
    pub fn como_mapa(&self) -> Option<&Rc<Vec<(Valor, Valor)>>> {
        match &self.estado {
            Estado::Mapa { entradas, .. } => Some(entradas),
            _ => None,
        }
    }

    pub fn como_funcao(&self) -> Option<Funcao> {
        match self.estado {
            Estado::Funcao { elemento, .. } => Some(elemento),
            _ => None,
        }
    }
}
