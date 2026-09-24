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
//! texto (`c:m`, `g:x`, `s:x`; privado com `@biblioteca`), o mesmo em
//! qualquer módulo, sem numeração combinada. A classe registra a tabela na
//! partida (`dartforge_registrar_metodos`).

use super::LlvmEmitter;
use crate::hir::*;
use std::fmt::Write;

/// O hash de um seletor (o mesmo de `lower::sdk_fonte::hash_seletor`).
pub fn hash_seletor(texto: &str) -> i64 {
    crate::lower::closures::hash_nome(texto)
}

/// Escapa bytes para um `c"…"` do LLVM.
fn bytes_llvm(s: &str) -> String {
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
    /// `%v = recv.<seletor>(args)`: a entrada vem de
    /// `dartforge_seletor(cache, recv, hash, nome, n)` — o cache do ponto de
    /// chamada (id de classe, entrada), a busca na tabela da classe na falha,
    /// e a entrada de `NoSuchMethodError` quando a classe não tem o membro.
    pub(super) fn emitir_chamada_por_seletor(
        &mut self,
        v: u32,
        seletor: &str,
        recv: &Operand,
        args: &[Operand],
        nomes: &[String],
    ) {
        let k = self.vetor_de[&Self::descritor(args.len(), nomes)];
        let n = args.len().max(1);
        for (i, a) in args.iter().enumerate() {
            let s = self.coagir(a, Type::I64);
            writeln!(self.out, "  %sa{v}_{i} = getelementptr [{n} x i64], ptr %sargs{v}, i64 0, i64 {i}").unwrap();
            writeln!(self.out, "  store i64 {s}, ptr %sa{v}_{i}").unwrap();
        }
        let r = self.coagir(recv, Type::Ref);
        let ic = self.caches_de_seletor;
        let nome = match self.nomes_de_seletor.iter().position(|s| s == seletor) {
            Some(j) => j,
            None => {
                self.nomes_de_seletor.push(seletor.to_string());
                self.nomes_de_seletor.len() - 1
            }
        };
        self.caches_de_seletor += 1;
        let h = hash_seletor(seletor);
        writeln!(
            self.out,
            "  %sf{v} = call ptr @dartforge_seletor(ptr @df.ic.{ic}, i64 {r}, i64 {h}, ptr @df.seln.{nome}, i64 {})",
            seletor.len()
        )
        .unwrap();
        writeln!(self.out, "  %v{v} = call i64 %sf{v}(i64 {r}, ptr %sargs{v}, ptr @df.arr.{k})").unwrap();
    }

    /// Os nomes das funções do rastro (`DARTFORGE_RASTRO=1`).
    pub(super) fn emitir_nomes_do_rastro(&mut self) {
        for (k, s) in std::mem::take(&mut self.nomes_do_rastro).iter().enumerate() {
            writeln!(self.out, "@df.rastro.{k} = private unnamed_addr constant [{} x i8] c\"{}\"", s.len(), bytes_llvm(s)).unwrap();
        }

    }

    /// Os caches dos pontos de chamada e os nomes dos seletores.
    pub(super) fn emitir_globais_de_seletores(&mut self) {
        if self.caches_de_seletor == 0 {
            return;
        }
        self.out.push_str("; Seletores: cache por ponto de chamada (id de classe, entrada) e nomes\n");
        for k in 0..self.caches_de_seletor {
            writeln!(self.out, "@df.ic.{k} = private global [2 x i64] zeroinitializer").unwrap();
        }
        for (j, s) in self.nomes_de_seletor.iter().enumerate() {
            writeln!(
                self.out,
                "@df.seln.{j} = private unnamed_addr constant [{} x i8] c\"{}\"",
                s.len(),
                bytes_llvm(s)
            )
            .unwrap();
        }
        self.out.push('\n');
    }

    /// A função `<registro>`: os nomes das classes do módulo, as arestas de
    /// subtipo e as tabelas de métodos, entregues ao runtime.
    pub(super) fn emitir_registro(&mut self, registro: &str) {
        let modulo = self.module;
        let mut corpo = String::new();
        for class in &modulo.classes {
            let idx = self.string_const_index(class.name.as_bytes()).unwrap_or(0);
            writeln!(
                corpo,
                "  call void @dartforge_register_class_name(i64 {}, ptr @.str.{idx}, i64 {})",
                class.id,
                class.name.len()
            )
            .unwrap();
        }
        for (sub, sup) in &modulo.subtyping_edges {
            writeln!(corpo, "  call void @dartforge_register_subclass(i64 {sub}, i64 {sup})").unwrap();
        }
        corpo.push_str(&self.emitir_tabelas_de_metodos());
        // As classes do programa (e as formas de record) registram a tabela
        // na partida. `_StackTrace` do SDK também precisa: o runtime cria o
        // primeiro trace diretamente, sem passar por `object_new_t`, que é o
        // ponto de registro preguiçoso das demais classes do SDK.
        for (cid, simbolo, _) in &modulo.tabelas_de_metodos {
            let trace_do_runtime = modulo.biblioteca_sdk && modulo.classes.iter()
                .any(|classe| classe.id == *cid && classe.name == "_StackTrace");
            if !modulo.biblioteca_sdk || trace_do_runtime {
                writeln!(corpo, "  call void @dartforge_registrar_tabela(i64 {cid}, ptr @{simbolo})").unwrap();
            }
        }
        for (k, (nome, simbolo)) in modulo.ajudantes.iter().enumerate() {
            writeln!(
                self.out,
                "@df.ajn.{k} = private unnamed_addr constant [{} x i8] c\"{}\"",
                nome.len(),
                bytes_llvm(nome)
            )
            .unwrap();
            writeln!(
                corpo,
                "  call void @dartforge_registrar_ajudante(ptr @df.ajn.{k}, i64 {}, ptr @{simbolo})",
                nome.len()
            )
            .unwrap();
        }
        writeln!(self.out, "define void @{registro}() {{\nb0:").unwrap();
        self.out.push_str(&corpo);
        self.out.push_str("  ret void\n}\n\n");
    }

    /// `dartforge_dispatch_toString` (o runtime pede o texto de um valor) pelo
    /// seletor `c:toString`: o `toString()` da classe dinâmica, do programa
    /// ou do SDK da fonte.
    pub(super) fn emitir_to_string_por_seletor(&mut self) {
        let s = "c:toString";
        let nome = match self.nomes_de_seletor.iter().position(|x| x == s) {
            Some(j) => j,
            None => {
                self.nomes_de_seletor.push(s.to_string());
                self.nomes_de_seletor.len() - 1
            }
        };
        let ic = self.caches_de_seletor;
        self.caches_de_seletor += 1;
        let k = self.vetor_de[&vec![0, 0]];
        let h = hash_seletor(s);
        writeln!(
            self.out,
            "define i64 @dartforge_dispatch_toString(i64 %obj) {{\nb0:\n  %a = alloca [1 x i64]\n  \
             %f = call ptr @dartforge_seletor(ptr @df.ic.{ic}, i64 %obj, i64 {h}, ptr @df.seln.{nome}, i64 {})\n  \
             %r = call i64 %f(i64 %obj, ptr %a, ptr @df.arr.{k})\n  ret i64 %r\n}}\n",
            s.len()
        )
        .unwrap();
    }

    /// As tabelas de métodos das classes do módulo, como dados, e a função
    /// pública de cada uma, `df.mt.<biblioteca>.<Classe>`, que devolve o
    /// endereço dela. A tabela é registrada no runtime **na primeira
    /// alocação** de um objeto da classe (`dartforge_object_new_t`): uma
    /// classe que o programa nunca instancia não tem a tabela alcançada, e o
    /// ligador (`/OPT:REF`, no perfil de produção) tira a tabela, os
    /// adaptadores e os métodos que só ela alcançava. As classes dos valores
    /// do runtime são registradas pela entrada do programa.
    fn emitir_tabelas_de_metodos(&mut self) -> String {
        let modulo = self.module;
        for (cid, simbolo, metodos) in &modulo.tabelas_de_metodos {
            let mut pares: Vec<(i64, &str, &str)> =
                metodos.iter().map(|(s, f)| (hash_seletor(s), s.as_str(), f.as_str())).collect();
            pares.sort();
            for par in pares.windows(2) {
                assert!(
                    par[0].0 != par[1].0 || par[0].1 == par[1].1,
                    "hash de seletor repetido na classe {cid}: {} e {}",
                    par[0].1,
                    par[1].1
                );
            }
            pares.dedup_by(|a, b| a.0 == b.0);
            for (_, _, f) in &pares {
                let s = f.to_string();
                self.anotar_externo(&s, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
            }
            let mut itens = vec![format!("i64 {cid}"), format!("i64 {}", pares.len())];
            itens.extend(pares.iter().map(|(h, _, f)| format!("i64 {h}, ptr @{f}")));
            let mut tipos = vec!["i64", "i64"];
            tipos.extend(pares.iter().flat_map(|_| ["i64", "ptr"]));
            writeln!(
                self.out,
                "@{simbolo}$d = private unnamed_addr constant {{ {} }} {{ {} }}",
                tipos.join(", "),
                itens.join(", ")
            )
            .unwrap();
            writeln!(self.out, "define ptr @{simbolo}() {{\nb0:\n  ret ptr @{simbolo}$d\n}}").unwrap();
        }
        String::new()
    }
    /// `define` de uma função que pode aparecer em mais de um módulo
    /// (entradas de tear-off, constantes canônicas): `linkonce_odr` num
    /// `comdat` próprio — o ligador fica com uma. Os outros símbolos não
    /// mudam.
    pub(super) fn ligacao_de(&mut self, simbolo: &str) -> (&'static str, String) {
        if !self.module.modo_sdk {
            return ("", String::new());
        }
        let compartilhado =
            simbolo.ends_with("$tear") || simbolo.ends_with("$tearm") || simbolo.starts_with("dfc.");
        if !compartilhado {
            return ("", String::new());
        }
        self.comdats.push(simbolo.to_string());
        ("linkonce_odr ", format!(" comdat($\"{simbolo}\")"))
    }

    /// Declarações do que o módulo usa e outro módulo define (P5c): funções
    /// Dart chamadas diretamente, entradas de closure, natives do runtime
    /// fora da tabela de externs. Sem o SDK da fonte o módulo é o programa
    /// inteiro, e nada sai aqui.
    pub(super) fn emitir_declaracoes_externas(&mut self) {
        self.emitir_nomes_do_rastro();
        for c in std::mem::take(&mut self.comdats) {
            writeln!(self.out, "$\"{c}\" = comdat any").unwrap();
        }
        let mut definidas: std::collections::HashSet<&str> =
            self.module.functions.iter().map(|f| f.symbol.as_str()).collect();
        // As funções das tabelas de métodos do módulo (`emitir_tabelas_de_metodos`).
        definidas.extend(self.module.tabelas_de_metodos.iter().map(|(_, s, _)| s.as_str()));
        let declaradas_runtime: std::collections::HashSet<&str> = super::externs::EXTERNS
            .iter()
            .filter_map(|e| {
                let i = e.decl.find('@')? + 1;
                let f = e.decl[i..].find('(')? + i;
                Some(&e.decl[i..f])
            })
            .collect();
        let mut saida = std::collections::BTreeMap::new();
        for (simbolo, decl) in &self.externos {
            if definidas.contains(simbolo.as_str()) || declaradas_runtime.contains(simbolo.as_str()) {
                continue;
            }
            // O que o próprio emissor escreve (`dartforge_dispatch_toString`,
            // `dartforge_entry`, os registros) não é externo.
            if (simbolo.starts_with("dartforge_") && !simbolo.starts_with("dartforge_nativo_"))
                || simbolo == "df.registrar.programa"
            {
                continue;
            }
            saida.insert(simbolo.clone(), decl.clone());
        }
        if saida.is_empty() {
            return;
        }
        self.out.push_str("; Definidos em outro módulo (SDK da fonte) ou no runtime\n");
        for (_, d) in saida {
            self.out.push_str(&d);
            self.out.push('\n');
        }
        self.out.push('\n');
    }

    /// Anota a assinatura de uma chamada a um símbolo, para declará-lo se
    /// ele não for definido no módulo.
    pub(super) fn anotar_externo(&mut self, simbolo: &str, ret: Type, args: &[Type]) {
        if self.externos.contains_key(simbolo) {
            return;
        }
        let a: Vec<&str> = args.iter().map(|t| t.llvm_ir()).collect();
        self.externos
            .insert(simbolo.to_string(), format!("declare {} @{simbolo}({})", ret.llvm_ir(), a.join(", ")));
    }
}
