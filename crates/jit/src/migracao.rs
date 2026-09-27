//! Migração das instâncias vivas numa recarga estrutural (J03).
//!
//! O emissor nativo escreve no IR o layout dos objetos de cada classe do
//! programa (`; df.campos <id> <base> nome:flags:tipo,…`, com `a` anulável e
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
pub(crate) struct Campo {
    pub nome: String,
    pub anulavel: bool,
    pub late: bool,
    pub tipo: String,
}

/// O layout de uma classe: id, campos de enum antes dos declarados, campos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    pub classe: i64,
    pub base: usize,
    pub campos: Vec<Campo>,
}

/// A migração dos objetos de uma classe: o novo número de campos e, para cada
/// posição nova, a antiga de onde vem o valor (`-1`: `null` ou não
/// inicializado).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Migracao {
    pub classe: i64,
    pub origem: Vec<i64>,
}

/// O inverso do `escapar` do emissor: `$` e dois dígitos hexadecimais por
/// byte UTF-8.
fn desescapar(parte: &str) -> Option<String> {
    let b = parte.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            v.push(u8::from_str_radix(parte.get(i + 1..i + 3)?, 16).ok()?);
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
pub(crate) fn layouts_do_ir(ir: &str) -> Vec<Layout> {
    ir.lines()
        .filter_map(|l| {
            let resto = l.strip_prefix("; df.campos ")?;
            let mut p = resto.splitn(3, ' ');
            let classe = p.next()?.parse().ok()?;
            let base = p.next()?.parse().ok()?;
            let lista = p.next().unwrap_or("");
            let campos = lista
                .split(',')
                .filter(|c| !c.is_empty())
                .map(|c| {
                    let mut q = c.split(':');
                    let nome = desescapar(q.next()?)?;
                    let flags = q.next()?;
                    let tipo = desescapar(q.next()?)?;
                    Some(Campo { nome, anulavel: flags.contains('a'), late: flags.contains('l'), tipo })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Layout { classe, base, campos })
        })
        .collect()
}

/// A migração das classes cujo layout mudou de `antes` para `depois`, ou a
/// mensagem da recusa. `nome` dá o nome de uma classe pelo id.
pub(crate) fn planejar(antes: &[Layout], depois: &[Layout], nome: &dyn Fn(i64) -> String) -> Result<Vec<Migracao>, String> {
    let mut plano = Vec::new();
    for velho in antes {
        let Some(novo) = depois.iter().find(|l| l.classe == velho.classe) else { continue };
        if novo == velho {
            continue;
        }
        let classe = nome(velho.classe);
        if novo.base != velho.base {
            return Err(format!("os campos de enum de {classe} mudaram; a recarga é recusada — reinicie a sessão"));
        }
        let mut origem: Vec<i64> = (0..novo.base as i64).collect();
        for c in &novo.campos {
            match velho.campos.iter().position(|v| v.nome == c.nome) {
                Some(i) => {
                    let v = &velho.campos[i];
                    if v.tipo != c.tipo {
                        return Err(format!(
                            "o campo `{}` de {classe} mudou de tipo ({} para {}); os objetos vivos guardam valores do tipo \
                             antigo, então a recarga é recusada — reinicie a sessão",
                            c.nome, v.tipo, c.tipo
                        ));
                    }
                    if v.late != c.late {
                        return Err(format!(
                            "o campo `{}` de {classe} {} `late`; a recarga é recusada — reinicie a sessão",
                            c.nome,
                            if c.late { "passou a ser" } else { "deixou de ser" }
                        ));
                    }
                    origem.push((velho.base + i) as i64);
                }
                None if c.anulavel || c.late => origem.push(-1),
                None => {
                    return Err(format!(
                        "o campo novo `{}` de {classe} ({}) não é anulável nem `late`: os objetos vivos não têm valor \
                         para ele, então a recarga é recusada — reinicie a sessão, ou declare-o `late` ou anulável",
                        c.nome, c.tipo
                    ));
                }
            }
        }
        plano.push(Migracao { classe: velho.classe, origem });
    }
    Ok(plano)
}

/// O plano no formato do runtime (`dartforge_definir_migracao`):
/// `[n, (classe, novo_len, origem…)…]`.
pub(crate) fn codificar(plano: &[Migracao]) -> Vec<i64> {
    let mut v = vec![plano.len() as i64];
    for m in plano {
        v.push(m.classe);
        v.push(m.origem.len() as i64);
        v.extend(&m.origem);
    }
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    fn campo(nome: &str, anulavel: bool, late: bool, tipo: &str) -> Campo {
        Campo { nome: nome.into(), anulavel, late, tipo: tipo.into() }
    }

    #[test]
    fn le_os_layouts_do_ir() {
        let ir = "; df.campos 7 0 valor::int,rotulo$5f:a:String$3f\n; df.campos 8 2 \ndefine void @f() { ret void }\n";
        let l = layouts_do_ir(ir);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].campos, vec![campo("valor", false, false, "int"), campo("rotulo_", true, false, "String?")]);
        assert_eq!((l[1].classe, l[1].base, l[1].campos.len()), (8, 2, 0));
    }

    #[test]
    fn migra_pelo_nome_e_aceita_anulavel_e_late() {
        let antes = [Layout { classe: 5, base: 0, campos: vec![campo("a", false, false, "int"), campo("b", false, false, "String")] }];
        let depois = [Layout {
            classe: 5,
            base: 0,
            campos: vec![campo("b", false, false, "String"), campo("c", true, false, "int?"), campo("d", false, true, "int")],
        }];
        let plano = planejar(&antes, &depois, &|_| "C".into()).expect("aceita");
        assert_eq!(plano, vec![Migracao { classe: 5, origem: vec![1, -1, -1] }]);
        assert_eq!(codificar(&plano), vec![1, 5, 3, 1, -1, -1]);
    }

    #[test]
    fn recusa_campo_novo_sem_valor_e_tipo_mudado() {
        let antes = [Layout { classe: 5, base: 0, campos: vec![campo("a", false, false, "int")] }];
        let novo = [Layout { classe: 5, base: 0, campos: vec![campo("a", false, false, "int"), campo("z", false, false, "int")] }];
        let erro = planejar(&antes, &novo, &|_| "C".into()).unwrap_err();
        assert!(erro.contains("o campo novo `z` de C (int) não é anulável nem `late`"), "{erro}");
        let tipo = [Layout { classe: 5, base: 0, campos: vec![campo("a", false, false, "String")] }];
        let erro = planejar(&antes, &tipo, &|_| "C".into()).unwrap_err();
        assert!(erro.contains("mudou de tipo (int para String)"), "{erro}");
    }
}
