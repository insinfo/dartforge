//! Assistências sobre membros de classe, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Convert to final field` | `refactor.convert.getterToFinalField` | `ConvertIntoFinalField` |
//! | `Convert to getter` | `refactor.convert.finalFieldToGetter` | `ConvertIntoGetter` |
//! | `Convert to normal parameter` | `refactor.convert.toConstructorNormalParameter` | `ConvertToNormalParameter` |
//! | `Convert class to a mixin` | `refactor.convert.classToMixin` | `ConvertClassToMixin` |
//! | `Encapsulate field` | `refactor.encapsulateField` | `EncapsulateField` |
//! | `Convert to field formal parameter` | `refactor.convert.toConstructorFieldParameter` | `ConvertToFieldParameter` |
//! | `Create a local variable that shadows the field` | `refactor.shadowField` | `ShadowField` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::Marca;
use crate::refatoracoes::Contexto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody, StmtKind};
use dartforge_types::TypeId;

/// As espécies de `TypeAnnotation`.
const TIPOS: &[&str] = &["NamedType", "GenericFunctionType", "RecordTypeAnnotation"];

impl Contexto<'_> {
    fn acao_de_membro(&self, uri: &str, titulo: &str, especie: &str, span: Span, texto: String) -> AcaoDeCodigo {
        AcaoDeCodigo {
            titulo: titulo.into(),
            especie: especie.into(),
            edicoes: vec![Edicao { uri: uri.to_string(), span, texto }],
            diagnostico: None,
            criar_arquivo: None,
        }
    }

    /// `ConvertIntoFinalField` (convert_into_final_field.dart).
    pub(crate) fn converter_em_campo_final(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        // O getter envolvente: passa por identificadores e tipos.
        let mut getter = None;
        for n in self.com_pais(no) {
            let e = self.especie(n);
            if e == "MethodDeclaration" {
                getter = Some(n);
                break;
            }
            if e == "SimpleIdentifier" || TIPOS.contains(&e) || e == "TypeArgumentList" {
                continue;
            }
            break;
        }
        let getter = getter?;
        let Marca::Funcao(fid) = self.arvore.nos[getter].marca else { return None };
        let f = self.ast.function(fid);
        if f.kind != ast::FunctionKind::Getter {
            return None;
        }
        let nome = f.name?;
        // Sem setter correspondente (`variable.setter2`).
        let elemento = self.p.funcao_do_no(self.unidade, fid)?;
        let prog = self.p.programa();
        let fe = prog.function(elemento);
        let chave = self.p.consulta.nomes.lookup(&format!("{}=", self.p.nome(fe.name)));
        let tem_setter = chave.is_some_and(|k| match (fe.class, fe.extension) {
            (Some(c), _) => {
                let k_ = prog.class(c);
                if fe.static_ { k_.static_members.contains_key(&k) } else { k_.instance_members.contains_key(&k) }
            }
            (None, Some(x)) => {
                let x_ = prog.extension(x);
                if fe.static_ { x_.static_members.contains_key(&k) } else { x_.instance_members.contains_key(&k) }
            }
            _ => false,
        });
        if tem_setter {
            return None;
        }
        // A expressão devolvida.
        let expressao = match &f.body {
            FunctionBody::Expression(e) => Some(*e),
            FunctionBody::Block(b) => match &self.ast.stmt(*b).kind {
                StmtKind::Block(cmds) if cmds.len() == 1 => match &self.ast.stmt(cmds[0]).kind {
                    StmtKind::Return(Some(e)) => Some(*e),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }?;
        let mut codigo = String::from("final");
        if let Some(t) = f.return_type {
            let s = self.ast.ty(t).span;
            codigo.push(' ');
            codigo.push_str(&self.fonte[s.start..s.end]);
        }
        codigo.push(' ');
        codigo.push_str(&self.fonte[nome.span.start..nome.span.end]);
        let es = self.ast.expr(expressao).span;
        if !matches!(self.ast.expr(expressao).kind, ast::ExprKind::Null) {
            codigo.push_str(" = ");
            codigo.push_str(&self.fonte[es.start..es.end]);
        }
        codigo.push(';');
        // `range.startEnd(returnType ?? propertyKeywordGet, getter)`.
        let inicio_troca = match f.return_type {
            Some(t) => self.ast.ty(t).span.start,
            None => self
                .tokens
                .iter()
                .filter(|t| t.span.end <= nome.span.start && &self.fonte[t.span.start..t.span.end] == "get")
                .map(|t| t.span.start)
                .next_back()?,
        };
        let faixa = Span { start: inicio_troca, end: self.arvore.nos[getter].fim };
        Some(self.acao_de_membro(uri, "Convert to final field", "refactor.convert.getterToFinalField", faixa, codigo))
    }

    /// `ConvertIntoGetter` (convert_into_getter.dart).
    pub(crate) fn converter_em_getter(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let mut declaracao = None;
        for n in self.com_pais(no) {
            let e = self.especie(n);
            if e == "FieldDeclaration" {
                declaracao = Some(n);
                break;
            }
            if matches!(e, "SimpleIdentifier" | "VariableDeclaration" | "VariableDeclarationList" | "TypeArgumentList") || TIPOS.contains(&e) {
                continue;
            }
            break;
        }
        let declaracao = declaracao?;
        let lista = self.filhos(declaracao).iter().copied().find(|&f| self.especie(f) == "VariableDeclarationList")?;
        let variaveis = self.filhos_da_especie(lista, "VariableDeclaration");
        if variaveis.len() != 1 {
            return None;
        }
        let campo = variaveis[0];
        // O `keyword` da lista (`final`, `const` ou `var`).
        let tipo = self.filhos(lista).iter().copied().find(|&f| TIPOS.contains(&self.especie(f)));
        let limite = tipo.map_or(self.arvore.nos[campo].inicio, |t| self.arvore.nos[t].inicio);
        let palavra = self
            .tokens
            .iter()
            .find(|t| t.span.start >= self.arvore.nos[lista].inicio && t.span.end <= limite && matches!(&self.fonte[t.span.start..t.span.end], "final" | "const" | "var"))?
            .span;
        let inicializador = *self.filhos(campo).first()?;
        let nome = self.token_seguinte(self.arvore.nos[campo].inicio)?;
        let mut codigo = String::new();
        if let Some(t) = tipo {
            codigo.push_str(self.texto_do_no(t));
            codigo.push(' ');
        }
        codigo.push_str("get ");
        codigo.push_str(&self.fonte[nome.start..nome.end]);
        codigo.push_str(" => ");
        codigo.push_str(self.texto_do_no(inicializador));
        codigo.push(';');
        let faixa = Span { start: palavra.start, end: self.arvore.nos[declaracao].fim };
        Some(self.acao_de_membro(uri, "Convert to getter", "refactor.convert.finalFieldToGetter", faixa, codigo))
    }

    /// `ConvertToNormalParameter` (convert_to_normal_parameter.dart): um
    /// `this.x` direto na lista de parâmetros de um construtor vira
    /// parâmetro comum com o inicializador `x = x`.
    pub(crate) fn converter_em_parametro_normal(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let parametro = self.arvore.localizar(inicio, fim)?;
        if self.especie(parametro) != "FieldFormalParameter" {
            return None;
        }
        let lista = self.pai(parametro)?;
        if self.especie(lista) != "FormalParameterList" {
            return None;
        }
        let construtor = self.pai(lista)?;
        if self.especie(construtor) != "ConstructorDeclaration" {
            return None;
        }
        // O nome: o último token do parâmetro (`this.x`).
        let nome_span = self.token_anterior(self.arvore.nos[parametro].fim)?;
        let nome = self.fonte[nome_span.start..nome_span.end].to_string();
        // O tipo do elemento: o do parâmetro do construtor, pela posição.
        let indice = self.filhos(lista).iter().position(|&k| k == parametro)?;
        let fim_do_no = self.arvore.nos[construtor].fim;
        let mi = self.ast.members.iter().position(|m| matches!(m.kind, ast::MemberKind::Constructor(_)) && m.span.end == fim_do_no)?;
        let elemento = self.p.construtor_do_no(self.unidade, ast::MemberId(mi as u32))?;
        let tipo = self.p.consulta.outline.functions.get(elemento.0 as usize)?.parameters.get(indice)?.ty;
        let mut importar = std::collections::BTreeSet::new();
        let substituto = if matches!(self.p.consulta.tabela.get(tipo), dartforge_types::Type::Dynamic) {
            nome.clone()
        } else {
            let mut escritor = crate::escrever_tipo::Escritor::novo(self, self.arvore.nos[parametro].inicio);
            let escrito = escritor.escrever(tipo, false).unwrap_or_default();
            importar = escritor.importar;
            format!("{escrito} {nome}")
        };
        let inicializadores: Vec<usize> = self
            .filhos(construtor)
            .iter()
            .copied()
            .filter(|&k| matches!(self.especie(k), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        let (em, texto) = match inicializadores.last() {
            None => (self.arvore.nos[lista].fim, format!(" : {nome} = {nome}")),
            Some(&u) => (self.arvore.nos[u].fim, format!(", {nome} = {nome}")),
        };
        let mut m = crate::refatoracoes_exec::Mudanca::default();
        m.adicionar(uri, self.arvore.span(parametro), substituto);
        m.adicionar(uri, Span { start: em, end: em }, texto);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to normal parameter".into(),
            especie: "refactor.convert.toConstructorNormalParameter".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `_SuperclassReferenceFinder`: a classe (não mixin) que declara cada
    /// membro alcançado por `super` dentro de `span`, com repetições.
    fn classes_referidas_por_super(&self, span: Span) -> Vec<dartforge_elements::model::ClassId> {
        use dartforge_types::{MemberRef, Resolved};
        let prog = self.p.programa();
        let mut saida = Vec::new();
        let dono = |r: Option<&Resolved>| -> Option<dartforge_elements::model::ClassId> {
            let f = match r? {
                Resolved::Member { member: MemberRef::Function(f), .. } => *f,
                Resolved::Member { member: MemberRef::Variable(v), .. } => prog.variable(*v).getter?,
                _ => return None,
            };
            let c = prog.function(f).class?;
            (prog.class(c).kind != dartforge_elements::model::ClassKind::Mixin).then_some(c)
        };
        let e_super = |x: ast::ExprId| matches!(self.ast.expr(x).kind, ast::ExprKind::Super);
        for (i, e) in self.ast.exprs.iter().enumerate() {
            if e.span.start < span.start || e.span.end > span.end {
                continue;
            }
            let id = ast::ExprId(i as u32);
            let alvo = match &e.kind {
                // `super.x`, `super.m(…)` (o nome está no `Property`).
                ast::ExprKind::Property { target, .. } if e_super(*target) => Some(id),
                ast::ExprKind::Binary { left, .. } if e_super(*left) => Some(id),
                ast::ExprKind::Index { target, .. } if e_super(*target) => Some(id),
                _ => None,
            };
            if let Some(c) = alvo.and_then(|x| dono(self.corpos.get_resolved(x))) {
                saida.push(c);
            }
        }
        saida
    }

    /// `ConvertClassToMixin` (convert_class_to_mixin.dart).
    pub(crate) fn converter_classe_em_mixin(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let declaracao = self.com_pais(no).find(|&k| self.especie(k) == "ClassDeclaration")?;
        let Marca::Decl(d) = self.arvore.nos[declaracao].marca else { return None };
        let ast::DeclKind::Class(k) = &self.ast.decl(d).kind else { return None };
        // `class` (a palavra) e o nome.
        let palavra_class = self
            .tokens
            .iter()
            .filter(|t| t.span.end <= k.name.span.start && &self.fonte[t.span.start..t.span.end] == "class")
            .map(|t| t.span)
            .next_back()?;
        if inicio > k.name.span.end || fim < palavra_class.start {
            return None;
        }
        if k.members.iter().any(|&m| matches!(self.ast.member(m).kind, ast::MemberKind::Constructor(_))) {
            return None;
        }
        if k.modifiers.final_ || k.modifiers.interface || k.modifiers.sealed {
            return None;
        }
        let classe = self.classe_da_declaracao(self.unidade, d)?;
        let dados = self.p.consulta.outline.classes.get(classe.0 as usize)?;
        let referidas = self.classes_referidas_por_super(self.arvore.span(declaracao));
        let tabela = &self.p.consulta.tabela;
        let classe_do_tipo = |t: TypeId| match tabela.get(t) {
            dartforge_types::Type::Interface { class, .. } => Some(*class),
            _ => None,
        };
        let mut restricoes: Vec<TypeId> = Vec::new();
        let mut interfaces: Vec<TypeId> = Vec::new();
        for &m in dados.mixins.iter() {
            if classe_do_tipo(m).is_some_and(|c| referidas.contains(&c)) {
                restricoes.push(m);
            } else {
                interfaces.push(m);
            }
        }
        if k.extends.is_some()
            && let Some(s) = dados.supertype
        {
            if referidas.len() > restricoes.len() {
                restricoes.insert(0, s);
            } else {
                interfaces.insert(0, s);
            }
        }
        interfaces.extend(dados.interfaces.iter().copied());
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, palavra_class.start);
        let mut texto = format!("mixin {}", &self.fonte[k.name.span.start..k.name.span.end]);
        // `writeTypeParameters2`.
        if !k.type_params.is_empty() {
            let mut partes = Vec::new();
            for (i, tp) in k.type_params.iter().enumerate() {
                let mut s = self.fonte[tp.name.span.start..tp.name.span.end].to_string();
                if tp.bound.is_some()
                    && let Some(&param) = dados.type_params.get(i)
                {
                    let limite = tabela.param(param).bound;
                    if let Some(b) = escritor.escrever(limite, false) {
                        s.push_str(" extends ");
                        s.push_str(&b);
                    }
                }
                partes.push(s);
            }
            texto.push('<');
            texto.push_str(&partes.join(", "));
            texto.push('>');
        }
        let escrever_lista = |escritor: &mut crate::escrever_tipo::Escritor<'_, '_>, tipos: &[TypeId], prefixo: &str, texto: &mut String| {
            if tipos.is_empty() {
                return;
            }
            let partes: Vec<String> = tipos.iter().map(|&t| escritor.escrever(t, false).unwrap_or_default()).collect();
            texto.push_str(prefixo);
            texto.push_str(&partes.join(", "));
        };
        escrever_lista(&mut escritor, &restricoes, " on ", &mut texto);
        escrever_lista(&mut escritor, &interfaces, " implements ", &mut texto);
        texto.push(' ');
        let inicio_troca = if k.modifiers.abstract_ {
            self.tokens
                .iter()
                .filter(|t| t.span.end <= palavra_class.start && &self.fonte[t.span.start..t.span.end] == "abstract")
                .map(|t| t.span.start)
                .next_back()
                .unwrap_or(palavra_class.start)
        } else {
            palavra_class.start
        };
        let abre = self.tokens.iter().find(|t| t.span.start >= k.name.span.end && &self.fonte[t.span.start..t.span.end] == "{")?.span.start;
        let mut m = crate::refatoracoes_exec::Mudanca::default();
        m.adicionar(uri, Span { start: inicio_troca, end: abre }, texto);
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &escritor.importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert class to a mixin".into(),
            especie: "refactor.convert.classToMixin".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `inheritanceManager.getOverridden4(classe, Name(nome))`: algum
    /// supertipo (superclasse, mixins, interfaces, restrições `on`, de forma
    /// transitiva) declara um membro de instância com esse nome — o getter ou
    /// o método (`setter` falso), ou o setter (`nome=`).
    fn membro_sobrescrito(&self, classe: dartforge_elements::model::ClassId, nome: &str, setter: bool) -> bool {
        let prog = self.p.programa();
        let chave = if setter { format!("{nome}_=") } else { nome.to_string() };
        let mut vistos = std::collections::HashSet::new();
        let ce = prog.class(classe);
        let mut pilha: Vec<dartforge_elements::model::ClassId> =
            ce.supertype_class.iter().chain(&ce.mixin_classes).chain(&ce.interface_classes).chain(&ce.on_classes).copied().collect();
        while let Some(c) = pilha.pop() {
            if !vistos.insert(c) {
                continue;
            }
            let e = prog.class(c);
            if e.instance_members.keys().any(|k| self.p.nome(*k) == chave) {
                return true;
            }
            pilha.extend(e.supertype_class.iter().chain(&e.mixin_classes).chain(&e.interface_classes).chain(&e.on_classes).copied());
        }
        false
    }

    /// `EncapsulateField` (encapsulate_field.dart): no nome de um campo de
    /// instância público, não final, único na declaração, de classe ou mixin,
    /// o campo vira privado e ganha getter e setter com o nome dele.
    pub(crate) fn encapsular_campo(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let declaracao = self.com_pais(no).find(|&k| self.especie(k) == "FieldDeclaration")?;
        let lista = self.filhos(declaracao).iter().copied().find(|&k| self.especie(k) == "VariableDeclarationList")?;
        // As palavras antes da lista (`static`, `covariant`…) e as da lista.
        let palavras = |de: usize, ate: usize| -> Vec<&str> {
            self.tokens.iter().filter(|t| t.span.start >= de && t.span.end <= ate).map(|t| &self.fonte[t.span.start..t.span.end]).collect()
        };
        let anotacoes: Vec<usize> = self.filhos(declaracao).iter().copied().filter(|&k| self.especie(k) == "Annotation").collect();
        let depois_dos_metadados = anotacoes.last().map_or(self.arvore.nos[declaracao].inicio, |&a| self.arvore.nos[a].fim);
        let antes = palavras(depois_dos_metadados, self.arvore.nos[lista].inicio);
        if antes.contains(&"static") {
            return None;
        }
        let tipo = self.filhos(lista).iter().copied().find(|&k| TIPOS.contains(&self.especie(k)));
        let variaveis: Vec<usize> = self.filhos(lista).iter().copied().filter(|&k| self.especie(k) == "VariableDeclaration").collect();
        let ate_o_nome = variaveis.first().map_or(self.arvore.nos[lista].fim, |&v| self.arvore.nos[v].inicio);
        let da_lista = palavras(self.arvore.nos[lista].inicio, ate_o_nome);
        let palavra = da_lista.iter().find(|w| matches!(**w, "var" | "final" | "const")).copied();
        if palavra.is_none() && tipo.is_none() {
            return None;
        }
        if palavra == Some("final") || variaveis.len() != 1 {
            return None;
        }
        let nome_tk = self.token_seguinte(self.arvore.nos[variaveis[0]].inicio)?;
        let nome = &self.fonte[nome_tk.start..nome_tk.end];
        if nome.starts_with('_') {
            return None;
        }
        // `nameToken != token`: o token da seleção é o nome.
        if !(nome_tk.start <= inicio && inicio <= nome_tk.end) {
            return None;
        }
        let corpo_da_classe = self.pai(declaracao)?;
        if !matches!(self.especie(corpo_da_classe), "ClassDeclaration" | "MixinDeclaration") {
            return None;
        }
        let Marca::Decl(d) = self.arvore.nos[corpo_da_classe].marca else { return None };
        let classe = self.classe_da_declaracao(self.unidade, d)?;
        let tx = crate::refatoracoes_exec::Texto::novo(self.fonte);
        let eol = tx.eol();
        let mut m = crate::refatoracoes_exec::Mudanca::default();
        // Sem as anotações no campo.
        if let (Some(&primeira), Some(&ultima)) = (anotacoes.first(), anotacoes.last()) {
            m.adicionar(uri, tx.faixa_de_linhas(self.arvore.nos[primeira].inicio, self.arvore.nos[ultima].fim), String::new());
        }
        m.adicionar(uri, nome_tk, format!("_{nome}"));
        let codigo_do_tipo = tipo.map_or(String::new(), |t| self.texto_do_no(t).to_string());
        // Os construtores: os `this.x` e os inicializadores do campo.
        for &membro in self.filhos(corpo_da_classe) {
            if self.especie(membro) != "ConstructorDeclaration" {
                continue;
            }
            let Some(&parametros) = self.filhos(membro).iter().find(|&&k| self.especie(k) == "FormalParameterList") else { continue };
            let inicializadores: Vec<usize> = self
                .filhos(membro)
                .iter()
                .copied()
                .filter(|&k| matches!(self.especie(k), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
                .collect();
            for &parametro in self.filhos(parametros) {
                let (interno, nomeado) = if self.especie(parametro) == "DefaultFormalParameter" {
                    let Some(&i) = self.filhos(parametro).first() else { continue };
                    // `isNamed`: entre `{ }`.
                    let antes_do_parametro = &self.fonte[self.arvore.nos[parametros].inicio..self.arvore.nos[parametro].inicio];
                    (i, antes_do_parametro.contains('{'))
                } else {
                    (parametro, false)
                };
                if self.especie(interno) != "FieldFormalParameter" {
                    continue;
                }
                let Some(identificador) = self.token_anterior(self.arvore.nos[interno].fim) else { continue };
                if &self.fonte[identificador.start..identificador.end] != nome {
                    continue;
                }
                if nomeado {
                    // `this.` sai (com o tipo do campo no lugar) e o campo é
                    // inicializado na lista.
                    let this_ = self.tokens.iter().find(|t| t.span.start >= self.arvore.nos[interno].inicio && &self.fonte[t.span.start..t.span.end] == "this")?.span;
                    let ponto = self.token_seguinte(this_.end)?;
                    let texto = if codigo_do_tipo.is_empty() { String::new() } else { format!("{codigo_do_tipo} ") };
                    m.adicionar(uri, Span { start: this_.start, end: ponto.end }, texto);
                    if inicializadores.is_empty() {
                        let fim_dos_parametros = self.arvore.nos[parametros].fim;
                        m.adicionar(uri, Span { start: fim_dos_parametros, end: fim_dos_parametros }, format!(" : _{nome} = {nome}"));
                    } else {
                        let separador = self.token_anterior(self.arvore.nos[inicializadores[0]].inicio)?;
                        m.adicionar(uri, Span { start: separador.end, end: separador.end }, format!(" _{nome} = {nome},"));
                    }
                    break;
                }
                m.adicionar(uri, identificador, format!("_{nome}"));
            }
            for &i in &inicializadores {
                if self.especie(i) != "ConstructorFieldInitializer" {
                    continue;
                }
                let Some(&campo) = self.filhos(i).first() else { continue };
                if self.texto_do_no(campo) == nome {
                    m.adicionar(uri, self.arvore.span(campo), format!("_{nome}"));
                }
            }
        }
        // O getter e o setter, depois da declaração.
        let documentacao = self.filhos(declaracao).iter().copied().find(|&k| self.especie(k) == "Comment").map(|c| self.texto_do_no(c).to_string());
        let codigo = if codigo_do_tipo.is_empty() { String::new() } else { format!("{codigo_do_tipo} ") };
        let cabecalho = |preservar_override: bool| -> String {
            let mut s = format!("{eol}{eol}");
            if let Some(doc) = &documentacao {
                s.push_str(&format!("  {doc}{eol}"));
            }
            for &a in &anotacoes {
                let texto = self.texto_do_no(a);
                if texto != "@override" || preservar_override {
                    s.push_str(&format!("  {texto}{eol}"));
                }
            }
            s
        };
        let mut s = cabecalho(self.membro_sobrescrito(classe, nome, false));
        s.push_str(&format!("  {codigo}get {nome} => _{nome};"));
        s.push_str(&cabecalho(self.membro_sobrescrito(classe, nome, true)));
        s.push_str(&format!("  set {nome}({codigo}value) {{{eol}    _{nome} = value;{eol}  }}"));
        let fim_da_declaracao = self.arvore.nos[declaracao].fim;
        m.adicionar(uri, Span { start: fim_da_declaracao, end: fim_da_declaracao }, s);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Encapsulate field".into(),
            especie: "refactor.encapsulateField".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `ConvertToFieldParameter._findParameter`: o parâmetro simples (direto
    /// na lista de um construtor) da seleção, ou o citado pela expressão de
    /// um inicializador de campo.
    fn parametro_para_campo(&self, no: usize) -> Option<(usize, Span, usize)> {
        let pai = self.pai(no)?;
        if self.especie(no) == "SimpleFormalParameter" {
            let nome = self.token_anterior(self.arvore.nos[no].fim)?;
            if self.especie(pai) != "FormalParameterList" {
                return None;
            }
            let construtor = self.pai(pai)?;
            return (self.especie(construtor) == "ConstructorDeclaration").then_some((no, nome, construtor));
        }
        if self.especie(no) == "SimpleIdentifier" && self.especie(pai) == "ConstructorFieldInitializer" {
            let construtor = self.pai(pai)?;
            if self.especie(construtor) != "ConstructorDeclaration" || self.filhos(pai).get(1) != Some(&no) {
                return None;
            }
            let lista = self.filhos(construtor).iter().copied().find(|&k| self.especie(k) == "FormalParameterList")?;
            let texto = self.texto_do_no(no);
            for &f in self.filhos(lista) {
                if self.especie(f) == "SimpleFormalParameter"
                    && let Some(nome) = self.token_anterior(self.arvore.nos[f].fim)
                    && &self.fonte[nome.start..nome.end] == texto
                {
                    return Some((f, nome, construtor));
                }
            }
        }
        None
    }

    /// `ConvertToFieldParameter` (convert_to_field_parameter.dart): o
    /// parâmetro citado uma só vez nos inicializadores, num `campo = p`, vira
    /// `this.campo` e o inicializador sai.
    pub(crate) fn converter_em_parametro_de_campo(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let (parametro, nome, construtor) = self.parametro_para_campo(no)?;
        let nome_do_parametro = &self.fonte[nome.start..nome.end];
        let inicializadores: Vec<usize> = self
            .filhos(construtor)
            .iter()
            .copied()
            .filter(|&k| matches!(self.especie(k), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        // `_ReferenceCounter`: os identificadores dos inicializadores cujo
        // elemento é o parâmetro.
        let mut contagem = 0;
        let mut pilha = inicializadores.clone();
        while let Some(k) = pilha.pop() {
            if self.especie(k) == "SimpleIdentifier"
                && let Marca::Expr(x) = self.arvore.nos[k].marca
                && match self.corpos.get_resolved(x) {
                    Some(dartforge_types::Resolved::Parameter { name, .. }) => self.p.nome(*name) == nome_do_parametro,
                    // Nos inicializadores o parâmetro é um local declarado no nome dele.
                    Some(dartforge_types::Resolved::Local(_)) => self.corpos.declaracao_local(x) == Some(nome.start),
                    _ => false,
                }
            {
                contagem += 1;
            }
            pilha.extend(self.filhos(k).iter().copied());
        }
        if contagem != 1 {
            return None;
        }
        let inicializador = inicializadores.iter().copied().filter(|&i| {
            self.especie(i) == "ConstructorFieldInitializer"
                && self.filhos(i).get(1).is_some_and(|&e| self.especie(e) == "SimpleIdentifier" && self.texto_do_no(e) == nome_do_parametro)
        }).last()?;
        let campo = self.texto_do_no(*self.filhos(inicializador).first()?).to_string();
        let mut edicoes = vec![(self.arvore.span(parametro), format!("this.{campo}"))];
        let indice = inicializadores.iter().position(|&i| i == inicializador)?;
        let lista = self.filhos(construtor).iter().copied().find(|&k| self.especie(k) == "FormalParameterList")?;
        if inicializadores.len() == 1 {
            edicoes.push((Span { start: self.arvore.nos[lista].fim, end: self.arvore.nos[inicializador].fim }, String::new()));
        } else if indice == 0 {
            edicoes.push((Span { start: self.arvore.nos[inicializador].inicio, end: self.arvore.nos[inicializadores[1]].inicio }, String::new()));
        } else {
            edicoes.push((Span { start: self.arvore.nos[inicializadores[indice - 1]].fim, end: self.arvore.nos[inicializador].fim }, String::new()));
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to field formal parameter".into(),
            especie: "refactor.convert.toConstructorFieldParameter".into(),
            edicoes: edicoes.into_iter().rev().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `ShadowField` (shadow_field.dart): o campo testado com `is` ou com
    /// `==`/`!=` na condição de um `if` (através de `&&`) dentro de um bloco
    /// que não escreve nele ganha um local de mesmo nome antes do `if`.
    pub(crate) fn sombrear_campo(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        use dartforge_types::{MemberRef, Resolved};
        let no = self.arvore.localizar(inicio, fim)?;
        if self.especie(no) != "SimpleIdentifier" {
            return None;
        }
        let Marca::Expr(x) = self.arvore.nos[no].marca else { return None };
        let prog = self.p.programa();
        // O getter de uma classe (o acessor do campo ou o declarado).
        let (variavel, getter, classe) = match self.corpos.get_resolved(x)? {
            Resolved::Member { member: MemberRef::Variable(v), .. } => (Some(*v), None, prog.variable(*v).class?),
            Resolved::Member { member: MemberRef::Function(f), .. } => {
                let fe = prog.function(*f);
                match fe.kind {
                    dartforge_elements::model::FunctionKind::Getter => (None, Some(*f), fe.class?),
                    // O getter sintético de um campo.
                    dartforge_elements::model::FunctionKind::ImplicitAccessor => {
                        let v = fe.variable?;
                        if prog.variable(v).getter != Some(*f) {
                            return None;
                        }
                        (Some(v), None, fe.class?)
                    }
                    _ => return None,
                }
            }
            _ => return None,
        };
        if self.escritas.contains(&x) {
            return None;
        }
        // `_getStatement`.
        let pai = self.pai(no)?;
        let condicao = match self.especie(pai) {
            "IsExpression" if self.filhos(pai).first() == Some(&no) => pai,
            "BinaryExpression" if matches!(self.operador_binario(pai), Some("==" | "!=")) => pai,
            _ => return None,
        };
        let mut acima = self.pai(condicao)?;
        while self.especie(acima) == "BinaryExpression" && self.operador_binario(acima) == Some("&&") {
            acima = self.pai(acima)?;
        }
        if self.especie(acima) != "IfStatement" {
            return None;
        }
        let comando = acima;
        let bloco = self.pai(comando)?;
        if self.especie(bloco) != "Block" {
            return None;
        }
        // `correspondingSetter2`.
        let nome = self.texto_do_no(no).to_string();
        let setter: Option<dartforge_elements::model::FunctionElementId> = match (variavel, getter) {
            (Some(v), _) => prog.variable(v).setter,
            (None, Some(_)) => prog.class(classe).instance_members.iter().find(|(k, _)| self.p.nome(**k) == format!("{nome}_=")).map(|(_, f)| *f),
            _ => None,
        };
        let setter = setter?;
        // `_ReferenceFinder`: uma escrita no setter dentro do bloco.
        let span_do_bloco = self.arvore.span(bloco);
        let escreve = self.escritas.iter().any(|&e| {
            let s = self.ast.expr(e).span;
            s.start >= span_do_bloco.start
                && s.end <= span_do_bloco.end
                && match self.corpos.get_resolved(e) {
                    Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => prog.variable(*v).setter == Some(setter),
                    Some(Resolved::Member { member: MemberRef::Function(f), .. }) => *f == setter,
                    _ => false,
                }
        });
        if escreve {
            return None;
        }
        let tx = crate::refatoracoes_exec::Texto::novo(self.fonte);
        let offset = self.arvore.nos[comando].inicio;
        let prefixo = tx.prefixo_da_linha(offset);
        let eol = tx.eol();
        Some(AcaoDeCodigo {
            titulo: "Create a local variable that shadows the field".into(),
            especie: "refactor.shadowField".into(),
            edicoes: vec![Edicao { uri: uri.to_string(), span: Span { start: offset, end: offset }, texto: format!("var {nome} = this.{nome};{eol}{prefixo}") }],
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
