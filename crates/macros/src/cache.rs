//! Cache limitado de planos próprios das macros incorporadas, sem ASTs ou fonte.
//!
//! A validação da classe acontece antes de `intern_validated`, inclusive em acertos.
//! O plano reaproveita apenas o esquema; AST, tipos aplicados e spans são novos a
//! cada expansão. Este cache não evita parsing, validação nem materialização.
//!
//! O esquema inclui quais macros a classe aplica, então `@JsonCodable()` e
//! `@DataClass()` sobre campos idênticos são entradas distintas e nunca se
//! confundem. `crate::PLAN_VERSION` acompanha essa forma.
use crate::{MacroPlan, PlanShape, PlannedField};
use dartforge_syntax::Class;
use std::{collections::VecDeque, mem::size_of, sync::Arc};

/// Contadores acumulados e tamanho atual da retenção de planos.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MacroCacheStats {
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub entries: usize,
    pub payload_bytes: usize,
}

/// Plano próprio e estimativa de seus bytes retidos, do menos ao mais recente.
struct Entry {
    plan: Arc<MacroPlan>,
    payload_bytes: usize,
}

/// Memoização LRU por esquema exato, limitada simultaneamente por entradas e bytes.
///
/// A estimativa inclui MacroPlan, capacidade do vetor de campos e capacidade de
/// seus nomes. Exclui estruturas da fila, contadores Arc e overhead do alocador;
/// não mede RSS nem planos Arc mantidos temporariamente pelo chamador. Não guarda
/// erros, nomes de classes, IDs de tipos ou classes, caminhos, spans ou ASTs.
/// As macros aplicadas são parte da chave e cabem nos bytes já contados por
/// MacroPlan, sem alterar a estimativa de payload.
pub struct MacroSession {
    entries: VecDeque<Entry>,
    max_entries: usize,
    max_payload_bytes: usize,
    stats: MacroCacheStats,
}

impl Default for MacroSession {
    /// Limita a sessão a 256 esquemas e um MiB de payload estimado.
    fn default() -> Self {
        Self::with_limits(256, 1024 * 1024)
    }
}

impl MacroSession {
    /// Define ambos os limites; zero em qualquer limite desativa a retenção.
    pub fn with_limits(max_entries: usize, max_payload_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            max_payload_bytes,
            stats: MacroCacheStats::default(),
        }
    }

    /// Retorna contadores acumulados; acerto do cache JS não acessa esta sessão.
    pub fn stats(&self) -> MacroCacheStats {
        self.stats
    }

    /// Descarta planos retidos e reinicia os contadores; Arcs externos continuam válidos.
    pub fn clear(&mut self) {
        self.entries = VecDeque::new();
        self.stats = MacroCacheStats::default();
    }

    /// Consulta um esquema já validado sem copiar nomes no caminho de acerto.
    ///
    /// O motor deve validar elegibilidade, colisões e construtor ANTES desta chamada.
    /// Tipos de campos devem ser os seis escalares permitidos, sem IDs. A chave
    /// cobre campos, construtor e as macros aplicadas; a comparação linear é
    /// limitada por max_entries e não depende de hashes.
    pub(crate) fn intern_validated(
        &mut self,
        class: &Class<'_>,
        shape: PlanShape,
    ) -> Arc<MacroPlan> {
        let found = self.entries.iter().position(|entry| {
            entry.plan.generate_constructor == shape.generate_constructor
                && entry.plan.json_codable == shape.json_codable
                && entry.plan.data_class == shape.data_class
                && entry.plan.fields.len() == class.fields.len()
                && entry
                    .plan
                    .fields
                    .iter()
                    .zip(&class.fields)
                    .all(|(planned, field)| {
                        planned.name == field.name
                            && planned.ty == field.ty
                            && planned.is_final == field.is_final
                    })
        });
        if let Some(index) = found {
            let entry = self
                .entries
                .remove(index)
                .expect("índice localizado na fila");
            let plan = Arc::clone(&entry.plan);
            self.entries.push_back(entry);
            self.stats.hits = self.stats.hits.saturating_add(1);
            return plan;
        }
        self.stats.misses = self.stats.misses.saturating_add(1);
        let plan = Arc::new(MacroPlan {
            fields: class
                .fields
                .iter()
                .map(|field| PlannedField {
                    name: field.name.to_owned(),
                    ty: field.ty,
                    is_final: field.is_final,
                })
                .collect(),
            generate_constructor: shape.generate_constructor,
            json_codable: shape.json_codable,
            data_class: shape.data_class,
        });
        let Some(payload_bytes) = plan_payload(&plan) else {
            return plan;
        };
        if self.max_entries == 0 || payload_bytes > self.max_payload_bytes {
            return plan;
        }
        while self.entries.len() >= self.max_entries
            || self.stats.payload_bytes > self.max_payload_bytes - payload_bytes
        {
            let Some(entry) = self.entries.pop_front() else {
                break;
            };
            self.stats.payload_bytes -= entry.payload_bytes;
            self.stats.evictions = self.stats.evictions.saturating_add(1);
        }
        self.entries.push_back(Entry {
            plan: Arc::clone(&plan),
            payload_bytes,
        });
        self.stats.payload_bytes += payload_bytes;
        self.stats.entries = self.entries.len();
        plan
    }
}

/// Soma capacidades sem permitir que overflow produza uma estimativa pequena.
fn plan_payload(plan: &MacroPlan) -> Option<usize> {
    let fields = plan
        .fields
        .capacity()
        .checked_mul(size_of::<PlannedField>())?;
    plan.fields.iter().try_fold(
        size_of::<MacroPlan>().checked_add(fields)?,
        |total, field| total.checked_add(field.name.capacity()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_syntax::Program;

    /// Analisa uma classe de teste; a validação de elegibilidade pertence ao motor.
    fn program(source: &str) -> Program<'_> {
        dartforge_parser::parse(&dartforge_lexer::lex(source).unwrap(), source.len()).unwrap()
    }

    /// Esquema de uma classe JsonCodable com construtor gerado, usado como padrão.
    fn json(generate_constructor: bool) -> PlanShape {
        PlanShape {
            generate_constructor,
            json_codable: true,
            data_class: false,
        }
    }

    /// Nomes/IDs/spans da classe e corpos alheios não são capturados pelo plano.
    #[test]
    fn borrowed_hit_reuses_owned_plan_after_source_drop() {
        let mut session = MacroSession::default();
        let first = {
            let source =
                String::from("class A{final String _name;int unrelated()=>1;}void main(){}");
            let unit = program(&source);
            session.intern_validated(&unit.classes[0], json(true))
        };
        let unit =
            program("class Other{} class B{final String _name;int unrelated()=>2;}void main(){}");
        let second = session.intern_validated(&unit.classes[1], json(true));
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(second.fields[0].name, "_name");
        assert_eq!(session.stats().hits, 1);
        assert_eq!(session.stats().misses, 1);
    }

    /// Tipo, ordem, nomes, final e modo do construtor fazem parte do esquema exato.
    #[test]
    fn relevant_schema_changes_miss() {
        let mut session = MacroSession::default();
        for source in [
            "class C{int a;String b;}void main(){}",
            "class C{String a;String b;}void main(){}",
            "class C{String b;int a;}void main(){}",
            "class C{int renamed;String b;}void main(){}",
            "class C{final int a;String b;}void main(){}",
        ] {
            session.intern_validated(&program(source).classes[0], json(true));
        }
        let unit = program("class C{int a;String b;}void main(){}");
        session.intern_validated(&unit.classes[0], json(false));
        assert_eq!(session.stats().misses, 6);
        assert_eq!(session.stats().hits, 0);
    }

    /// Campos idênticos com macros diferentes são entradas distintas do cache.
    #[test]
    fn applied_macros_are_part_of_the_key() {
        let unit = program("class C{final int a;}void main(){}");
        let mut session = MacroSession::default();
        let shapes = [(true, false), (false, true), (true, true)];
        let mut plans = Vec::new();
        for (json_codable, data_class) in shapes {
            plans.push(session.intern_validated(
                &unit.classes[0],
                PlanShape {
                    generate_constructor: true,
                    json_codable,
                    data_class,
                },
            ));
        }
        assert_eq!(session.stats().misses, 3);
        assert_eq!(session.stats().hits, 0);
        for (plan, (json_codable, data_class)) in plans.iter().zip(shapes) {
            assert_eq!(plan.json_codable, json_codable);
            assert_eq!(plan.data_class, data_class);
        }
        for (json_codable, data_class) in shapes {
            session.intern_validated(
                &unit.classes[0],
                PlanShape {
                    generate_constructor: true,
                    json_codable,
                    data_class,
                },
            );
        }
        assert_eq!(session.stats().hits, 3);
        assert_eq!(session.stats().misses, 3);
    }

    /// Hits atualizam LRU; planos externos sobrevivem à expulsão e clear reinicia tudo.
    #[test]
    fn entry_and_payload_limits_evict_without_invalidating_external_plans() {
        let a = program("class A{int a;}void main(){}");
        let b = program("class B{int b;}void main(){}");
        let c = program("class C{int c;}void main(){}");
        let mut session = MacroSession::with_limits(2, usize::MAX);
        let retained = session.intern_validated(&a.classes[0], json(true));
        session.intern_validated(&b.classes[0], json(true));
        assert!(Arc::ptr_eq(
            &retained,
            &session.intern_validated(&a.classes[0], json(true))
        ));
        session.intern_validated(&c.classes[0], json(true));
        assert_eq!(session.stats().evictions, 1);
        assert!(Arc::ptr_eq(
            &retained,
            &session.intern_validated(&a.classes[0], json(true))
        ));
        session.intern_validated(&b.classes[0], json(true));
        assert_eq!(session.stats().evictions, 2);
        let budget = plan_payload(&retained).unwrap();
        let mut bounded = MacroSession::with_limits(10, budget);
        bounded.intern_validated(&a.classes[0], json(true));
        bounded.intern_validated(&b.classes[0], json(true));
        assert_eq!(bounded.stats().entries, 1);
        assert_eq!(bounded.stats().evictions, 1);
        assert!(bounded.stats().payload_bytes <= budget);
        session.clear();
        assert_eq!(session.stats(), MacroCacheStats::default());
        assert_eq!(retained.fields[0].name, "a");
    }

    /// Zero e entrada maior que o orçamento não impedem a preparação do plano.
    #[test]
    fn disabled_and_oversized_entries_are_not_retained() {
        let unit = program("class C{String veryLongFieldName;}void main(){}");
        for (entries, bytes) in [(0, usize::MAX), (10, 0), (10, 1)] {
            let mut session = MacroSession::with_limits(entries, bytes);
            for _ in 0..2 {
                session.intern_validated(&unit.classes[0], json(true));
            }
            assert_eq!(session.stats().misses, 2);
            assert_eq!(session.stats().entries, 0);
            assert_eq!(session.stats().payload_bytes, 0);
        }
    }
}
