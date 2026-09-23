//! A tabela de natives do SDK da fonte (P5b, docs/NATIVO-PLANO.md §7).
//!
//! No SDK da VM, um membro `external` sem patch é implementado de um de dois
//! jeitos: **native** — `@pragma("vm:external-name", "Nome")`, uma função C++
//! do runtime da VM (`runtime/lib/*.cc`) — ou **intrínseco** —
//! `@pragma("vm:recognized", ...)` sem nome de native, que o compilador da
//! VM gera no lugar da chamada (aritmética de `double`, `[]` de `_List`,
//! campos invisíveis de `_GrowableList`/`_compact_hash`/`Record`…). O nativo
//! faz o mesmo: todo native das [`BIBLIOTECAS_DA_FONTE`] tem uma entrada
//! aqui, com o que o backend faz com ele.
//!
//! * [`Estado::Runtime`]: uma função do runtime Rust,
//!   `dartforge_nativo_<Nome>` (o fragmento `nativos` de `crates/runtime`),
//!   com a assinatura na representação da HIR (R1) dos tipos Dart
//!   declarados: `int` → `i64`, `double` → `double`, `bool` → `i8`, o resto
//!   (receptor inclusive) → `Ref`. Erros saem pela exceção pendente.
//! * [`Estado::Pendente`]: ainda sem implementação; o lowering de P5d
//!   transforma a chamada em diagnóstico (N1), nunca em chamada a símbolo
//!   inexistente.
//!
//! Os efeitos seguem G8 (NATIVO-PLANO §6.5): marcados de forma conservadora
//! (aloca, lança) até um teste sob `DARTFORGE_GC_STRESS=1` provar o
//! contrário.
//!
//! O teste `inventario::toda_native_da_fonte_tem_entrada` carrega o SDK com a
//! sobreposição, percorre os `external` sem patch das bibliotecas da fonte e
//! recusa native sem entrada (e entrada que não é mais native): é o que
//! impede um patch novo do SDK — ou da sobreposição — de entrar em silêncio.
//!
//! [`BIBLIOTECAS_DA_FONTE`]: crate::sdk_modulo::BIBLIOTECAS_DA_FONTE

/// O que o backend faz com um native.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Função do runtime `dartforge_nativo_<Nome>`.
    Runtime,
    /// Sem implementação ainda: chamar é diagnóstico.
    Pendente,
}

/// Efeitos de uma chamada (G8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Efeitos {
    pub aloca: bool,
    pub lanca: bool,
    pub chama_dart: bool,
}

/// Conservador: aloca e lança, não chama Dart.
pub const CONSERVADOR: Efeitos = Efeitos { aloca: true, lanca: true, chama_dart: false };

/// Um native de `@pragma("vm:external-name", nome)`.
#[derive(Debug, Clone, Copy)]
pub struct Nativo {
    pub nome: &'static str,
    pub estado: Estado,
    pub efeitos: Efeitos,
}

const fn runtime(nome: &'static str) -> Nativo {
    Nativo { nome, estado: Estado::Runtime, efeitos: CONSERVADOR }
}

const fn pendente(nome: &'static str) -> Nativo {
    Nativo { nome, estado: Estado::Pendente, efeitos: CONSERVADOR }
}

/// Todo native das bibliotecas da fonte com a sobreposição `sdk_nativo/`,
/// em ordem de bytes (a de `str`, que a busca binária usa). Os `DartForge_*` são os da sobreposição.
pub const NATIVOS: &[Nativo] = &[
    pendente("AbstractType_equality"),
    pendente("AbstractType_getHashCode"),
    pendente("AbstractType_toString"),
    pendente("AssertionError_throwNew"),
    pendente("AssertionError_throwNewSource"),
    pendente("Bool_fromEnvironment"),
    pendente("Bool_hasEnvironment"),
    pendente("ClassID_getID"),
    pendente("Closure_computeHash"),
    pendente("Closure_equals"),
    pendente("DartForge_Timer_cancelar"),
    pendente("DartForge_Timer_novo"),
    pendente("DartForge_scheduleImmediate"),
    pendente("DateTime_currentTimeMicros"),
    pendente("DateTime_timeZoneName"),
    pendente("DateTime_timeZoneOffsetInSeconds"),
    runtime("Double_add"),
    runtime("Double_div"),
    runtime("Double_doubleFromInteger"),
    runtime("Double_equal"),
    runtime("Double_equalToInteger"),
    runtime("Double_flipSignBit"),
    runtime("Double_getIsInfinite"),
    runtime("Double_getIsNaN"),
    runtime("Double_getIsNegative"),
    runtime("Double_greaterThan"),
    runtime("Double_greaterThanFromInteger"),
    runtime("Double_mul"),
    pendente("Double_parse"),
    runtime("Double_sub"),
    runtime("Double_toString"),
    pendente("Double_toStringAsExponential"),
    pendente("Double_toStringAsFixed"),
    pendente("Double_toStringAsPrecision"),
    pendente("Error_throwWithStackTrace"),
    pendente("Error_trySetStackTrace"),
    pendente("FinalizerEntry_allocate"),
    pendente("Function_apply"),
    pendente("GrowableList_allocate"),
    pendente("GrowableList_getCapacity"),
    pendente("GrowableList_getLength"),
    pendente("GrowableList_setData"),
    pendente("GrowableList_setIndexed"),
    pendente("GrowableList_setLength"),
    pendente("Identical_comparison"),
    pendente("ImmutableList_from"),
    runtime("Integer_addFromInteger"),
    runtime("Integer_bitAndFromInteger"),
    runtime("Integer_bitOrFromInteger"),
    runtime("Integer_bitXorFromInteger"),
    runtime("Integer_equalToInteger"),
    pendente("Integer_fromEnvironment"),
    runtime("Integer_greaterThanFromInteger"),
    runtime("Integer_moduloFromInteger"),
    runtime("Integer_mulFromInteger"),
    runtime("Integer_shlFromInteger"),
    runtime("Integer_shrFromInteger"),
    runtime("Integer_subFromInteger"),
    runtime("Integer_truncDivFromInteger"),
    runtime("Integer_ushrFromInteger"),
    pendente("Internal_allocateObjectInstructionsEnd"),
    pendente("Internal_allocateObjectInstructionsStart"),
    runtime("Internal_allocateOneByteString"),
    runtime("Internal_allocateTwoByteString"),
    pendente("Internal_boundsCheckForPartialInstantiation"),
    pendente("Internal_collectAllGarbage"),
    pendente("Internal_deoptimizeFunctionsOnStack"),
    pendente("Internal_extractTypeArguments"),
    pendente("Internal_loadDynamicModule"),
    pendente("Internal_makeFixedListUnmodifiable"),
    pendente("Internal_makeListFixedLength"),
    pendente("Internal_nativeEffect"),
    pendente("Internal_prependTypeArguments"),
    pendente("Internal_unsafeCast"),
    runtime("Internal_writeIntoOneByteString"),
    runtime("Internal_writeIntoTwoByteString"),
    pendente("InvocationMirror_unpackTypeArguments"),
    pendente("LibraryPrefix_isLoaded"),
    pendente("LibraryPrefix_issueLoad"),
    pendente("LibraryPrefix_loadingUnit"),
    pendente("LibraryPrefix_setLoaded"),
    pendente("List_allocate"),
    pendente("List_getLength"),
    pendente("List_setIndexed"),
    pendente("List_slice"),
    runtime("Mint_bitLength"),
    runtime("Mint_bitNegate"),
    pendente("NoSuchMethodError_existingMethodSignature"),
    pendente("Object_equals"),
    pendente("Object_getHash"),
    pendente("Object_haveSameRuntimeType"),
    pendente("Object_instanceOf"),
    pendente("Object_runtimeType"),
    pendente("Object_simpleInstanceOf"),
    pendente("Object_toString"),
    pendente("OneByteString_allocateFromOneByteList"),
    runtime("OneByteString_substringUnchecked"),
    pendente("Random_initialSeed"),
    pendente("RegExp_ExecuteMatch"),
    pendente("RegExp_ExecuteMatchSticky"),
    pendente("RegExp_factory"),
    pendente("RegExp_getGroupCount"),
    pendente("RegExp_getGroupNameMap"),
    pendente("RegExp_getIsCaseSensitive"),
    pendente("RegExp_getIsDotAll"),
    pendente("RegExp_getIsMultiLine"),
    pendente("RegExp_getIsUnicode"),
    pendente("RegExp_getPattern"),
    pendente("SecureRandom_getBytes"),
    runtime("Smi_bitLength"),
    runtime("Smi_bitNegate"),
    pendente("StackTrace_current"),
    pendente("Stopwatch_frequency"),
    pendente("Stopwatch_now"),
    pendente("StringBase_createFromCodePoints"),
    pendente("StringBase_intern"),
    pendente("StringBase_joinReplaceAllResult"),
    runtime("StringBase_substringUnchecked"),
    pendente("StringBuffer_createStringFromUint16Array"),
    runtime("String_charAt"),
    runtime("String_concat"),
    pendente("String_concatRange"),
    pendente("String_fromEnvironment"),
    runtime("String_getHashCode"),
    runtime("String_getLength"),
    runtime("String_toLowerCase"),
    runtime("String_toUpperCase"),
    pendente("TwoByteString_allocateFromTwoByteList"),
    pendente("TypeError_throwNew"),
    pendente("Type_equality"),
    pendente("Uri_isWindowsPlatform"),
    pendente("WeakProperty_getKey"),
    pendente("WeakProperty_getValue"),
    pendente("WeakProperty_setKey"),
    pendente("WeakProperty_setValue"),
    pendente("WeakReference_getTarget"),
    pendente("WeakReference_setTarget"),
];

/// A entrada de um native, se existe.
pub fn nativo(nome: &str) -> Option<&'static Nativo> {
    NATIVOS.binary_search_by(|n| n.nome.cmp(nome)).ok().map(|i| &NATIVOS[i])
}

/// O símbolo do runtime que implementa um native.
pub fn simbolo(nome: &str) -> String {
    format!("dartforge_nativo_{nome}")
}

/// Um `external` sem patch das bibliotecas da fonte: o que o implementa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDoSdk {
    /// `dart:core`…
    pub biblioteca: String,
    /// `Classe.membro` ou `membro` (de topo).
    pub membro: String,
    /// O nome do native, se é um.
    pub native: Option<String>,
    /// Tem `@pragma("vm:recognized", ...)` (intrínseco da VM).
    pub reconhecido: bool,
}

/// Os `external` sem patch das bibliotecas dadas, com o pragma que os
/// implementa. Um `external` que tem patch não entra: quem o implementa é o
/// membro do patch (que entra, se for `external` também).
pub fn inventario(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    bibliotecas: &[&str],
) -> Vec<ExternalDoSdk> {
    use dartforge_elements::model::FunctionRef;
    use dartforge_frontend::ast;

    let mut saida = Vec::new();
    for f in &program.functions {
        if !f.external || f.patched_by.is_some() {
            continue;
        }
        let lib = program.library(f.library);
        let Some(nome_lib) = lib.uri.strip_prefix("dart:") else { continue };
        if !bibliotecas.contains(&nome_lib) {
            continue;
        }
        // O pragma mora no `Member`/`Decl` que declara a função.
        let metadata: &[ast::Annotation] = match f.node {
            FunctionRef::Function { unit, function } => {
                let a = &program.unit(unit).ast;
                a.members
                    .iter()
                    .find(|m| matches!(m.kind, ast::MemberKind::Method(id) if id == function))
                    .map(|m| &*m.metadata)
                    .or_else(|| {
                        a.decls
                            .iter()
                            .find(|d| matches!(d.kind, ast::DeclKind::Function(id) if id == function))
                            .map(|d| &*d.metadata)
                    })
                    .unwrap_or(&[])
            }
            FunctionRef::Constructor { unit, member } => &program.unit(unit).ast.members[member.0 as usize].metadata,
            FunctionRef::None => &[],
        };
        let unit = match f.node {
            FunctionRef::Function { unit, .. } | FunctionRef::Constructor { unit, .. } => Some(unit),
            FunctionRef::None => None,
        };
        let mut native = None;
        let mut reconhecido = false;
        for an in metadata {
            let Some(u) = unit else { break };
            if an.name.len() != 1 || interner.resolve(an.name[0].sym) != "pragma" {
                continue;
            }
            let Some(args) = &an.arguments else { continue };
            let textos: Vec<Option<String>> = args
                .args
                .iter()
                .map(|a| match &program.unit(u).ast.exprs[a.value.0 as usize].kind {
                    ast::ExprKind::String(lit) => {
                        lit.constant_value().map(|t| String::from_utf8_lossy(t.as_bytes()).into_owned())
                    }
                    _ => None,
                })
                .collect();
            match textos.first().and_then(|t| t.as_deref()) {
                Some("vm:external-name") => native = textos.get(1).cloned().flatten(),
                Some("vm:recognized") => reconhecido = true,
                _ => {}
            }
        }
        let dono = f.class.map(|c| interner.resolve(program.classes[c.0 as usize].name).to_string());
        let nome = interner.resolve(f.name).to_string();
        saida.push(ExternalDoSdk {
            biblioteca: lib.uri.clone(),
            membro: match dono {
                Some(c) => format!("{c}.{nome}"),
                None => nome,
            },
            native,
            reconhecido,
        });
    }
    saida.sort_by(|a, b| (&a.biblioteca, &a.membro).cmp(&(&b.biblioteca, &b.membro)));
    saida
}

#[cfg(test)]
mod inventario {
    use super::*;
    use std::path::Path;

    const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

    #[test]
    fn a_tabela_esta_em_ordem_e_sem_repeticao() {
        for par in NATIVOS.windows(2) {
            assert!(par[0].nome < par[1].nome, "{} antes de {}", par[0].nome, par[1].nome);
        }
        assert!(nativo("String_getLength").is_some());
        assert!(nativo("Inexistente").is_none());
    }

    /// Todo native da fonte (com a sobreposição) tem entrada, e toda entrada
    /// ainda é um native da fonte.
    #[test]
    fn toda_native_da_fonte_tem_entrada() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let externos = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let sdk = crate::sdk_modulo::carregar_sdk_nativo(Path::new(SDK)).unwrap();
                let tmp = tempfile::tempdir().unwrap();
                let entrada = tmp.path().join("main.dart");
                let mut fonte = String::new();
                for b in crate::sdk_modulo::BIBLIOTECAS_DA_FONTE {
                    fonte.push_str(&format!("import 'dart:{b}';\n"));
                }
                fonte.push_str("void main() {}\n");
                std::fs::write(&entrada, fonte).unwrap();
                let mut interner = dartforge_intern::Interner::new();
                let (program, diags) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
                assert!(diags.is_empty(), "{diags:?}");
                inventario(&program, &interner, crate::sdk_modulo::BIBLIOTECAS_DA_FONTE)
            })
            .unwrap()
            .join()
            .unwrap();
        let nomes: std::collections::BTreeSet<&str> = externos.iter().filter_map(|e| e.native.as_deref()).collect();
        let sem_entrada: Vec<&&str> = nomes.iter().filter(|n| nativo(n).is_none()).collect();
        assert!(sem_entrada.is_empty(), "natives da fonte sem entrada em NATIVOS: {sem_entrada:?}");
        let sobrando: Vec<&str> = NATIVOS.iter().map(|n| n.nome).filter(|n| !nomes.contains(n)).collect();
        assert!(sobrando.is_empty(), "entradas de NATIVOS que não são mais natives da fonte: {sobrando:?}");
        // Um `external` sem patch sem native nem `vm:recognized` não tem
        // implementação em lugar nenhum — nem na VM.
        let orfaos: Vec<String> = externos
            .iter()
            .filter(|e| e.native.is_none() && !e.reconhecido)
            .map(|e| format!("{}::{}", e.biblioteca, e.membro))
            .collect();
        println!("externals: {} natives distintos, {} intrínsecos, {} sem pragma", nomes.len(), externos.iter().filter(|e| e.native.is_none() && e.reconhecido).count(), orfaos.len());
        for o in &orfaos {
            println!("  sem pragma: {o}");
        }
    }
}
