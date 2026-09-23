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
            let idx = self.string_const_index(&class.name).unwrap_or(0);
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

    /// As tabelas de métodos das classes do módulo, como dados: para cada
    /// classe, um vetor `[hash, entrada]…` ordenado pelo hash. Devolve as
    /// chamadas de registro.
    fn emitir_tabelas_de_metodos(&mut self) -> String {
        let modulo = self.module;
        let tabelas = &modulo.tabelas_de_metodos;
        let mut corpo = String::new();
        for (idx, (cid, metodos)) in tabelas.iter().enumerate() {
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
            if pares.is_empty() {
                continue;
            }
            for (_, _, f) in &pares {
                let simbolo = f.to_string();
                self.anotar_externo(&simbolo, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
            }
            let itens: Vec<String> = pares.iter().map(|(h, _, f)| format!("i64 {h}, ptr @{f}")).collect();
            let tipos: Vec<&str> = pares.iter().flat_map(|_| ["i64", "ptr"]).collect();
            writeln!(
                self.out,
                "@df.mt.{idx} = private unnamed_addr constant {{ {} }} {{ {} }}",
                tipos.join(", "),
                itens.join(", ")
            )
            .unwrap();
            writeln!(
                corpo,
                "  call void @dartforge_registrar_metodos(i64 {cid}, ptr @df.mt.{idx}, i64 {})",
                pares.len()
            )
            .unwrap();
        }
        corpo
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
        for c in std::mem::take(&mut self.comdats) {
            writeln!(self.out, "$\"{c}\" = comdat any").unwrap();
        }
        let definidas: std::collections::HashSet<&str> =
            self.module.functions.iter().map(|f| f.symbol.as_str()).collect();
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
