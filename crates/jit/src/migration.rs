//! Migração das instâncias vivas numa recarga estrutural (J03).
//!
//! O emissor nativo escreve no IR o layout dos objetos de cada classe do
//! programa (`; df.fields <id> <base> name:flags:tipo,…`, com `a` anulável e
//! `l` late; `base` são os campos antes dos declarados, os de enum). Quando
//! uma classe muda de campos, a recarga não precisa mais ser recusada: cada
//! objeto vivo dela passa ao layout novo **pelo nome** do campo, na
//! publicação — com todos os isolados parados no ponto seguro de evento, sem
//! quadro Dart na pilha —, e o que ele não tem recebe:
//!
//! * `null`, se o tipo do campo novo aceita `null`;
//! * nada (fica não inicializado), se o campo novo é `late`: a leitura roda o
//!   inicializador dele, como a VM faz com um campo acrescentado, ou lança o
//!   `LateInitializationError`.
//!
//! O resto é recusado com a mensagem exata, sem alterar a sessão: campo novo
//! que não é anulável nem `late` (o objeto vivo não tem valor para ele), campo
//! que mudou de tipo ou de `late` (o valor guardado pode não caber), e
//! mudança nos campos de enum.

/// Um campo do layout escrito no IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    pub name: String,
    pub nullable: bool,
    pub late: bool,
    pub tipo: String,
}

/// O layout de uma classe: id, campos de enum antes dos declarados, campos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    pub class: i64,
    pub base: usize,
    pub fields: Vec<Field>,
}

/// A migração dos objetos de uma classe: o novo número de campos e, para cada
/// posição nova, a antiga de onde vem o valor (`-1`: `null` ou não
/// inicializado).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Migration {
    pub class: i64,
    pub origin: Vec<i64>,
}

/// O inverso do `escapar` do emissor: `$` e dois dígitos hexadecimais por
/// byte UTF-8.
fn unescape(part: &str) -> Option<String> {
    let b = part.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            v.push(u8::from_str_radix(part.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            v.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(v).ok()
}

/// Os layouts escritos no IR `ir` (vazio num IR sem eles: a comparação fica
/// com a contagem de campos das alocações).
pub(crate) fn ir_layouts(ir: &str) -> Vec<Layout> {
    ir.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("; df.campos ")?;
            let mut p = rest.splitn(3, ' ');
            let class = p.next()?.parse().ok()?;
            let base = p.next()?.parse().ok()?;
            let list = p.next().unwrap_or("");
            let fields = list
                .split(',')
                .filter(|c| !c.is_empty())
                .map(|c| {
                    let mut q = c.split(':');
                    let name = unescape(q.next()?)?;
                    let flags = q.next()?;
                    let tipo = unescape(q.next()?)?;
                    Some(Field { name, nullable: flags.contains('a'), late: flags.contains('l'), tipo })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Layout { class, base, fields })
        })
        .collect()
}

/// A migração das classes cujo layout mudou de `before` para `after`, ou a
/// mensagem da recusa. `name` dá o nome de uma classe pelo id.
pub(crate) fn plan_fn(before: &[Layout], after: &[Layout], name: &dyn Fn(i64) -> String) -> Result<Vec<Migration>, String> {
    let mut plan = Vec::new();
    for old_gen in before {
        let Some(new) = after.iter().find(|l| l.class == old_gen.class) else { continue };
        if new == old_gen {
            continue;
        }
        let class = name(old_gen.class);
        if new.base != old_gen.base {
            return Err(format!("os campos de enum de {class} mudaram; a recarga é recusada — reinicie a sessão"));
        }
        let mut origin: Vec<i64> = (0..new.base as i64).collect();
        for c in &new.fields {
            match old_gen.fields.iter().position(|v| v.name == c.name) {
                Some(i) => {
                    let v = &old_gen.fields[i];
                    if v.tipo != c.tipo {
                        return Err(format!(
                            "o campo `{}` de {class} mudou de tipo ({} para {}); os objetos vivos guardam valores do tipo \
                             antigo, então a recarga é recusada — reinicie a sessão",
                            c.name, v.tipo, c.tipo
                        ));
                    }
                    if v.late != c.late {
                        return Err(format!(
                            "o campo `{}` de {class} {} `late`; a recarga é recusada — reinicie a sessão",
                            c.name,
                            if c.late { "passou a ser" } else { "deixou de ser" }
                        ));
                    }
                    origin.push((old_gen.base + i) as i64);
                }
                None if c.nullable || c.late => origin.push(-1),
                None => {
                    return Err(format!(
                        "o campo novo `{}` de {class} ({}) não é anulável nem `late`: os objetos vivos não têm valor \
                         para ele, então a recarga é recusada — reinicie a sessão, ou declare-o `late` ou anulável",
                        c.name, c.tipo
                    ));
                }
            }
        }
        plan.push(Migration { class: old_gen.class, origin });
    }
    Ok(plan)
}

/// O plano no formato do runtime (`dartforge_definir_migracao`):
/// `[n, (class, novo_len, origin…)…]`.
pub(crate) fn encode(plan: &[Migration]) -> Vec<i64> {
    let mut v = vec![plan.len() as i64];
    for m in plan {
        v.push(m.class);
        v.push(m.origin.len() as i64);
        v.extend(&m.origin);
    }
    v
}

#[cfg(test)]
mod tests_mod {
    use super::*;

    fn field(name: &str, nullable: bool, late: bool, tipo: &str) -> Field {
        Field { name: name.into(), nullable, late, tipo: tipo.into() }
    }

    #[test]
    fn le_os_layouts_do_ir() {
        let ir = "; df.campos 7 0 valor::int,rotulo$5f:a:String$3f\n; df.campos 8 2 \ndefine void @f() { ret void }\n";
        let l = ir_layouts(ir);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].fields, vec![field("valor", false, false, "int"), field("rotulo_", true, false, "String?")]);
        assert_eq!((l[1].class, l[1].base, l[1].fields.len()), (8, 2, 0));
    }

    #[test]
    fn migrates_by_name_and_accepts_nullable_and_late() {
        let before = [Layout { class: 5, base: 0, fields: vec![field("a", false, false, "int"), field("b", false, false, "String")] }];
        let after = [Layout {
            class: 5,
            base: 0,
            fields: vec![field("b", false, false, "String"), field("c", true, false, "int?"), field("d", false, true, "int")],
        }];
        let plan = plan_fn(&before, &after, &|_| "C".into()).expect("aceita");
        assert_eq!(plan, vec![Migration { class: 5, origin: vec![1, -1, -1] }]);
        assert_eq!(encode(&plan), vec![1, 5, 3, 1, -1, -1]);
    }

    #[test]
    fn rejects_new_field_without_value_and_changed_type() {
        let before = [Layout { class: 5, base: 0, fields: vec![field("a", false, false, "int")] }];
        let new = [Layout { class: 5, base: 0, fields: vec![field("a", false, false, "int"), field("z", false, false, "int")] }];
        let error = plan_fn(&before, &new, &|_| "C".into()).unwrap_err();
        assert!(error.contains("o campo novo `z` de C (int) não é anulável nem `late`"), "{error}");
        let tipo = [Layout { class: 5, base: 0, fields: vec![field("a", false, false, "String")] }];
        let error = plan_fn(&before, &tipo, &|_| "C".into()).unwrap_err();
        assert!(error.contains("mudou de tipo (int para String)"), "{error}");
    }
}
