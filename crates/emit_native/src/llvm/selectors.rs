//! Emissão do SDK da fonte (P5c, δ): a chamada por seletor, as tabelas de
//! métodos das classes e as declarações do que mora em outro módulo.
//!
//! **Por que tabelas por classe e não a tabela global de despacho.** A
//! *global dispatch table* da VM AOT (deslocamento por seletor + id de
//! classe) supõe o programa inteiro: os deslocamentos são escolhidos vendo
//! todas as classes. Com o SDK compilado **uma vez**, antes de qualquer
//! programa, os deslocamentos do SDK não podem depender das classes do
//! programa, e cada programa teria de redefinir um global por seletor que o
//! SDK usa (milhares) — ou o COFF teria de resolver símbolos fracos, que ele
//! não resolve do jeito que o ELF resolve. O que a VM faz no JIT resolve o
//! mesmo problema: cada classe tem a sua tabela (seletor → código), a busca
//! acontece na primeira chamada de um ponto e fica num cache do ponto
//! (*inline cache*). O seletor é identificado pelo hash FNV-1a de 64 bits do
//! texto (`c:m`, `g:x`, `s:x`; privado com `@library`), o mesmo em
//! qualquer módulo, sem numeração combinada. A classe registra a tabela na
//! partida (`dartforge_registrar_metodos`).

use super::LlvmEmitter;
use crate::hir::*;
use std::fmt::Write;

/// O hash de um seletor (o mesmo de `lower::sdk_source::selector_hash`).
pub fn selector_hash(text: &str) -> i64 {
    crate::lower::closures::hash_name(text)
}

/// Escapa bytes para um `c"…"` do LLVM.
pub(super) fn llvm_bytes(s: &str) -> String {
    let mut t = String::new();
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b == b' ' || b == b'_' || b == b'.' || b == b':' || b == b'@' {
            t.push(b as char);
        } else {
            t.push_str(&format!("\\{b:02X}"));
        }
    }
    t
}

impl LlvmEmitter<'_> {
    /// `%v = recv.<selector>(args)`: a entrada vem de
    /// `dartforge_seletor(cache, recv, hash, name, n)` — o cache do ponto de
    /// chamada (id de classe, entrada), a busca na tabela da classe na falha,
    /// e a entrada de `NoSuchMethodError` quando a classe não tem o membro.
    pub(super) fn emit_selector_call(
        &mut self,
        v: u32,
        selector: &str,
        recv: &Operand,
        args: &[Operand],
        names: &[String],
        tuple_types: &Operand,
    ) {
        let k = self.vector_of[&Self::descriptor(args.len(), names)];
        let n = args.len() + 1;
        for (i, a) in args.iter().enumerate() {
            let s = self.coerce(a, Type::I64);
            writeln!(self.out, "  %sa{v}_{i} = getelementptr [{n} x i64], ptr %sargs{v}, i64 0, i64 {i}").unwrap();
            writeln!(self.out, "  store i64 {s}, ptr %sa{v}_{i}").unwrap();
        }
        let tuple = self.coerce(tuple_types, Type::I64);
        writeln!(self.out, "  %sat{v} = getelementptr [{n} x i64], ptr %sargs{v}, i64 0, i64 {}", args.len()).unwrap();
        writeln!(self.out, "  store i64 {tuple}, ptr %sat{v}").unwrap();
        let r = self.coerce(recv, Type::Ref);
        let name = match self.selector_names.iter().position(|s| s == selector) {
            Some(j) => j,
            None => {
                self.selector_names.push(selector.to_string());
                self.selector_names.len() - 1
            }
        };
        self.selector_caches += 1;
        let h = selector_hash(selector);
        let slot = self.cache_slot();
        writeln!(self.out, "  %sic{v} = getelementptr i64, ptr %area, i64 {slot}").unwrap();
        if self.compact_selector {
            // O hash, o nome e o comprimento num descritor estático
            // (`@df.seld.<k>`): três argumentos no lugar de cinco.
            writeln!(self.out, "  %sf{v} = call ptr @df.seletor_d(ptr %sic{v}, i64 {r}, ptr @df.seld.{name})").unwrap();
        } else {
            writeln!(
                self.out,
                "  %sf{v} = call ptr @df.seletor(ptr %sic{v}, i64 {r}, i64 {h}, ptr @df.seln.{name}, i64 {})",
                selector.len()
            )
            .unwrap();
        }
        if self.arc_uniform_abi() {
            writeln!(self.out, "  %sc{v} = ptrtoint ptr %sf{v} to i64").unwrap();
            writeln!(self.out, "  %sav{v} = ptrtoint ptr %sargs{v} to i64").unwrap();
            writeln!(self.out, "  %sdv{v} = ptrtoint ptr @df.arr.{k} to i64").unwrap();
            writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_invocar_uniforme_v1(i64 %sc{v}, i64 {r}, i64 %sav{v}, i64 %sdv{v})").unwrap();
        } else {
            writeln!(self.out, "  %v{v} = call i64 %sf{v}(i64 {r}, ptr %sargs{v}, ptr @df.arr.{k})").unwrap();
        }
    }

    /// `%v = recv.<selector>` com o vetor de argumentos e o descritor de uma
    /// entrada uniforme repassados como vieram (`CallSelectorForward`); a
    /// busca da entrada é a de [`Self::emit_selector_call`].
    pub(super) fn emit_selector_forward(
        &mut self,
        v: u32,
        selector: &str,
        recv: &Operand,
        args: &Operand,
        desc: &Operand,
        arc_owned: bool,
    ) {
        let r = self.coerce(recv, Type::Ref);
        let a = self.operand_str(args);
        let d = self.operand_str(desc);
        let name = match self.selector_names.iter().position(|s| s == selector) {
            Some(j) => j,
            None => {
                self.selector_names.push(selector.to_string());
                self.selector_names.len() - 1
            }
        };
        self.selector_caches += 1;
        let h = selector_hash(selector);
        let slot = self.cache_slot();
        writeln!(self.out, "  %sic{v} = getelementptr i64, ptr %area, i64 {slot}").unwrap();
        if self.compact_selector {
            // O hash, o nome e o comprimento num descritor estático
            // (`@df.seld.<k>`): três argumentos no lugar de cinco.
            writeln!(self.out, "  %sf{v} = call ptr @df.seletor_d(ptr %sic{v}, i64 {r}, ptr @df.seld.{name})").unwrap();
        } else {
            writeln!(
                self.out,
                "  %sf{v} = call ptr @df.seletor(ptr %sic{v}, i64 {r}, i64 {h}, ptr @df.seln.{name}, i64 {})",
                selector.len()
            )
            .unwrap();
        }
        if arc_owned {
            assert!(self.module.arc_memory && self.module.arc_aot_ports.is_some(),
                "repasse ARC exige portas AOT certificadas");
            writeln!(self.out, "  %sc{v} = ptrtoint ptr %sf{v} to i64").unwrap();
            writeln!(self.out, "  %sav{v} = ptrtoint ptr {a} to i64").unwrap();
            writeln!(self.out, "  %sdv{v} = ptrtoint ptr {d} to i64").unwrap();
            writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_invocar_uniforme_v1(i64 %sc{v}, i64 {r}, i64 %sav{v}, i64 %sdv{v})").unwrap();
        } else {
            writeln!(self.out, "  %v{v} = call i64 %sf{v}(i64 {r}, ptr {a}, ptr {d})").unwrap();
        }
    }

    /// Os nomes das funções do rastro (`DARTFORGE_RASTRO=1`).
    pub(super) fn emit_stack_trace_names(&mut self) {
        for (k, s) in std::mem::take(&mut self.stack_trace_names).iter().enumerate() {
            writeln!(self.out, "@df.rastro.{k} = private unnamed_addr constant [{} x i8] c\"{}\"", s.len(), llvm_bytes(s)).unwrap();
        }

    }

    /// Os caches dos pontos de chamada e os nomes dos seletores.
    pub(super) fn emit_selector_globals(&mut self) {
        if self.selector_caches == 0 {
            return;
        }
        // Os caches por ponto de chamada (id de classe, entrada) moram na
        // área de globais do isolado; aqui só os nomes dos seletores.
        for (j, s) in self.selector_names.iter().enumerate() {
            writeln!(
                self.out,
                "@df.seln.{j} = private unnamed_addr constant [{} x i8] c\"{}\"",
                s.len(),
                llvm_bytes(s)
            )
            .unwrap();
            if self.compact_selector {
                writeln!(
                    self.out,
                    "@df.seld.{j} = private unnamed_addr constant {{ i64, ptr, i64 }} {{ i64 {}, ptr @df.seln.{j}, i64 {} }}",
                    selector_hash(s),
                    s.len()
                )
                .unwrap();
            }
        }
        self.out.push('\n');
    }

    /// A função `<record>`: os nomes das classes do módulo, as arestas de
    /// subtipo e as tabelas de métodos, entregues ao runtime.
    pub(super) fn emit_registration(&mut self, record: &str) {
        let module = self.module;
        let mut body = String::new();
        if module.arc_aot_ports.is_some() {
            body.push_str("  call void @df.arc.registrar_portas()\n");
        }
        for class in &module.classes {
            let idx = self.string_const_index(class.name.as_bytes()).unwrap_or(0);
            writeln!(
                body,
                "  call void @dartforge_register_class_name(i64 {}, ptr @.str.{idx}, i64 {})",
                class.id,
                class.name.len()
            )
            .unwrap();
        }
        for (sub, sup) in &module.subtyping_edges {
            writeln!(body, "  call void @dartforge_register_subclass(i64 {sub}, i64 {sup})").unwrap();
        }
        body.push_str(&self.emit_method_tables());
        // A seção dos objetos estáticos desta imagem (docs/NATIVO-ESPACO-UNIFICADO.md
        // §2.11): cada módulo com estáticos registra a da imagem em que foi
        // ligado (o executável, a DLL do SDK); o registro é idempotente.
        if self.static_objects {
            let (globals, start, end) = crate::alvo::image_markers();
            self.out.push_str(&globals);
            writeln!(body, "  call void @dartforge_registrar_imagem(ptr {start}, ptr {end})").unwrap();
        }
        // Raízes por mapas: os mapas de pilha da imagem em que este módulo
        // foi ligado (o registro é idempotente, por imagem), pelo formato
        // dela. O JIT acha e registra o mapa de cada objeto.
        if self.maps && !self.maps_in_jit {
            body.push_str(Self::maps_registration_call());
        }
        // O rastro simbólico (§13.14): a tabela da imagem, idempotente.
        if let Some(c) = self.trace_registration_call() {
            body.push_str(&c);
            body.push_str(&self.trace_field_registration_calls());
        }
        // As classes do programa (e as formas de record) registram a tabela
        // na partida. `_StackTrace` do SDK também precisa: o runtime cria o
        // primeiro trace diretamente, sem passar por `object_new_t`, que é o
        // ponto de registro preguiçoso das demais classes do SDK.
        for (cid, symbol, _) in &module.method_tables {
            let runtime_trace = module.sdk_library && module.classes.iter()
                .any(|cls| cls.id == *cid && cls.name == "_StackTrace");
            if !module.sdk_library || runtime_trace {
                writeln!(body, "  call void @dartforge_registrar_tabela(i64 {cid}, ptr @{symbol})").unwrap();
                if self.tables_at_link {
                    self.annotate_external(symbol, Type::Ptr, &[]);
                }
            }
        }
        for (k, (name, symbol)) in module.helpers.iter().enumerate() {
            writeln!(
                self.out,
                "@df.ajn.{k} = private unnamed_addr constant [{} x i8] c\"{}\"",
                name.len(),
                llvm_bytes(name)
            )
            .unwrap();
            writeln!(
                body,
                "  call void @dartforge_registrar_ajudante(ptr @df.ajn.{k}, i64 {}, ptr @{symbol})",
                name.len()
            )
            .unwrap();
        }
        for (k, name) in self.argument_names.iter().enumerate() {
            writeln!(
                self.out,
                "@df.nomearg.{k} = private unnamed_addr constant [{} x i8] c\"{}\"",
                name.len(),
                llvm_bytes(name)
            )
            .unwrap();
            writeln!(
                body,
                "  call void @dartforge_registrar_nome_de_argumento(ptr @df.nomearg.{k}, i64 {})",
                name.len()
            )
            .unwrap();
        }
        writeln!(self.out, "define void @{record}() {{\nb0:").unwrap();
        self.out.push_str(&body);
        self.out.push_str("  ret void\n}\n\n");
    }

    /// `dartforge_dispatch_toString` (o runtime pede o texto de um valor) pelo
    /// seletor `c:toString`: o `toString()` da classe dinâmica, do programa
    /// ou do SDK da fonte.
    pub(super) fn emit_to_string_by_selector(&mut self) {
        let s = "c:toString";
        let name = match self.selector_names.iter().position(|x| x == s) {
            Some(j) => j,
            None => {
                self.selector_names.push(s.to_string());
                self.selector_names.len() - 1
            }
        };
        self.selector_caches += 1;
        let k = self.vector_of[&vec![0, 0]];
        let h = selector_hash(s);
        self.current_function = "dartforge_dispatch_toString".to_string();
        self.cache_in_function = 0;
        let slot = self.cache_slot();
        writeln!(
            self.out,
            "define i64 @dartforge_dispatch_toString(i64 %obj) {{\nb0:\n  %a = alloca [1 x i64]\n  \
             store i64 0, ptr %a\n  \
             %area = call ptr @df.obter_area()\n  \
             %ic = getelementptr i64, ptr %area, i64 {slot}\n  \
             %f = call ptr @df.seletor(ptr %ic, i64 %obj, i64 {h}, ptr @df.seln.{name}, i64 {})\n  \
             %r = call i64 %f(i64 %obj, ptr %a, ptr @df.arr.{k})\n  ret i64 %r\n}}\n",
            s.len()
        )
        .unwrap();
    }

    /// As tabelas de métodos das classes do módulo, como dados, e a função
    /// pública de cada uma, `df.mt.<library>.<Classe>`, que devolve o
    /// endereço dela. A tabela é registrada no runtime **na primeira
    /// alocação** de um objeto da classe (`dartforge_object_new_t`): uma
    /// classe que o programa nunca instancia não tem a tabela alcançada, e o
    /// ligador (`/OPT:REF`, no perfil de produção) tira a tabela, os
    /// adaptadores e os métodos que só ela alcançava. As classes dos valores
    /// do runtime são registradas pela entrada do programa.
    ///
    /// Com as tabelas na ligação (o SDK de produção,
    /// docs/NATIVO-PODA-DE-TABELAS.md §3.2) nada sai aqui: o módulo do
    /// programa define as tabelas do SDK, só com os pares vivos
    /// (`pruning::assemble`).
    fn emit_method_tables(&mut self) -> String {
        if self.tables_at_link {
            return String::new();
        }
        let module = self.module;
        for (cid, symbol, methods) in &module.method_tables {
            let pairs = crate::pruning::table_pairs(*cid, methods);
            for (_, _, f) in &pairs {
                self.annotate_external(f, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
            }
            let items: Vec<(i64, String)> = pairs.iter().map(|(h, _, f)| (*h, format!("@{f}"))).collect();
            self.out.push_str(&crate::pruning::table_line(&format!("@{symbol}$d = private unnamed_addr constant "), i64::from(*cid), &items));
            writeln!(self.out, "define ptr @{symbol}() {{\nb0:\n  ret ptr @{symbol}$d\n}}").unwrap();
        }
        String::new()
    }
    /// `define` de uma função que pode aparecer em mais de um módulo
    /// (entradas de tear-off, constantes canônicas): `linkonce_odr` num
    /// `comdat` próprio — o ligador fica com uma. Os outros símbolos não
    /// mudam.
    pub(super) fn linkage_of(&mut self, symbol: &str) -> (&'static str, String) {
        if !self.module.sdk_mode {
            return ("", String::new());
        }
        let shared = if self.module.arc_memory {
            self.module.arc_shared_functions.contains(symbol)
        } else {
            symbol.ends_with("$tear") || symbol.ends_with("$tearm") || symbol.starts_with("dfc.")
        };
        if !shared {
            return ("", String::new());
        }
        if !crate::alvo::has_comdat() {
            // Mach-O: a definição `linkonce_odr` já é fraca, sem `comdat`.
            return ("linkonce_odr ", String::new());
        }
        self.comdats.push(symbol.to_string());
        ("linkonce_odr ", format!(" comdat($\"{symbol}\")"))
    }

    /// Declarações do que o módulo usa e outro módulo define (P5c): funções
    /// Dart chamadas diretamente, entradas de closure, natives do runtime
    /// fora da tabela de externs. Sem o SDK da fonte o módulo é o programa
    /// inteiro, e nada sai aqui.
    /// Os thunks Owned genéricos (`optimize::arc::natives`): a base roda num
    /// escopo nativo (os tokens que portas Dart devolvem ficam nele), e o
    /// resultado ganha um owner do código antes de o escopo fechar; com
    /// exceção pendente, null sem owner — o mesmo protocolo de
    /// `owner_em_escopo_nativo` no runtime.
    fn emit_owned_thunks(&mut self) {
        let thunks = std::mem::take(&mut self.owned_thunks);
        for (name, (base, types)) in thunks {
            let params: Vec<String> = types.iter().enumerate().map(|(i, t)| format!("{} %a{i}", t.llvm_ir())).collect();
            let params = params.join(", ");
            let _ = write!(
                self.out,
                "define internal i64 @{name}({params}) {{\n\
                 entrada:\n\
                 \x20 %e = call i64 @dartforge_arc_escopo_nativo_abrir_v1()\n\
                 \x20 %r = call i64 @{base}({params})\n\
                 \x20 %p = call i8 @dartforge_exception_pending()\n\
                 \x20 %z = icmp ne i8 %p, 0\n\
                 \x20 br i1 %z, label %fim, label %ok\n\
                 ok:\n\
                 \x20 call void @dartforge_arc_retain(i64 %r)\n\
                 \x20 br label %fim\n\
                 fim:\n\
                 \x20 %res = phi i64 [ 0, %entrada ], [ %r, %ok ]\n\
                 \x20 call void @dartforge_arc_escopo_nativo_fechar_v1(i64 %e)\n\
                 \x20 ret i64 %res\n\
                 }}\n"
            );
        }
    }

    /// Os thunks de escopo nativo (`optimize::arc::natives`): a base roda
    /// num escopo nativo, e o resultado (escalar, emprestado ou nenhum) e a
    /// pendência ficam como a base os deixou.
    fn emit_scope_thunks(&mut self) {
        let thunks = std::mem::take(&mut self.scope_thunks);
        for (name, (base, types, ret)) in thunks {
            let params: Vec<String> = types.iter().enumerate().map(|(i, t)| format!("{} %a{i}", t.llvm_ir())).collect();
            let params = params.join(", ");
            let r = ret.llvm_ir();
            let (call, ret_val) = if ret == Type::Void {
                (format!("call void @{base}({params})"), "ret void".to_string())
            } else {
                (format!("%r = call {r} @{base}({params})"), format!("ret {r} %r"))
            };
            let _ = write!(
                self.out,
                "define internal {r} @{name}({params}) {{\n\
                 entrada:\n\
                 \x20 %e = call i64 @dartforge_arc_escopo_nativo_abrir_v1()\n\
                 \x20 {call}\n\
                 \x20 call void @dartforge_arc_escopo_nativo_fechar_v1(i64 %e)\n\
                 \x20 {ret_val}\n\
                 }}\n"
            );
        }
    }

    pub(super) fn emit_external_declarations(&mut self) {
        self.emit_owned_thunks();
        self.emit_scope_thunks();
        self.emit_stack_trace_names();
        // `DARTFORGE_EFEITOS=check`: os nomes das externs conferidas.
        for (k, s) in std::mem::take(&mut self.checked_externs).iter().enumerate() {
            writeln!(self.out, "@df.efn.{k} = private unnamed_addr constant [{} x i8] c\"{}\"", s.len(), llvm_bytes(s)).unwrap();
        }
        for c in std::mem::take(&mut self.comdats) {
            writeln!(self.out, "$\"{c}\" = comdat any").unwrap();
        }
        let mut defined: std::collections::HashSet<&str> =
            self.module.functions.iter().map(|f| f.symbol.as_str()).collect();
        // As funções das tabelas de métodos do módulo (`emit_method_tables`).
        // Com as tabelas na ligação, elas são do módulo do programa.
        if !self.tables_at_link {
            defined.extend(self.module.method_tables.iter().map(|(_, s, _)| s.as_str()));
        }
        let runtime_declared: std::collections::HashSet<&str> = super::externs::EXTERNS
            .iter()
            .filter_map(|e| {
                let i = e.decl.find('@')? + 1;
                let f = e.decl[i..].find('(')? + i;
                Some(&e.decl[i..f])
            })
            .collect();
        let mut exit = std::collections::BTreeMap::new();
        for (symbol, decl) in &self.externals {
            if defined.contains(symbol.as_str()) || runtime_declared.contains(symbol.as_str()) {
                continue;
            }
            // O que o próprio emissor escreve (`dartforge_dispatch_toString`,
            // `dartforge_entry`, os registros) não é externo.
            if (symbol.starts_with("dartforge_") && !symbol.starts_with("dartforge_nativo_"))
                || symbol == "df.registrar.programa"
            {
                continue;
            }
            exit.insert(symbol.clone(), decl.clone());
        }
        if exit.is_empty() {
            return;
        }
        self.out.push_str("; Definidos em outro módulo (SDK da fonte) ou no runtime\n");
        for (_, d) in exit {
            self.out.push_str(&d);
            self.out.push('\n');
        }
        self.out.push('\n');
    }

    /// Anota a assinatura de uma chamada a um símbolo, para declará-lo se
    /// ele não for definido no módulo.
    pub(super) fn annotate_external(&mut self, symbol: &str, ret: Type, args: &[Type]) {
        if self.externals.contains_key(symbol) {
            return;
        }
        let a: Vec<&str> = args.iter().map(|t| t.llvm_ir()).collect();
        self.externals
            .insert(symbol.to_string(), format!("declare {} @{symbol}({})", ret.llvm_ir(), a.join(", ")));
    }
}
