//! Unidade de compilação, diretivas e declarações de topo e de membro
//! (`parseUnit`, `parseTopLevelDeclarationImpl`,
//! `parseTopLevelKeywordDeclaration`, `parseTopLevelMemberImpl`,
//! `parseClassOrMixinOrExtensionOrEnumMemberImpl`, `parseMethod`,
//! `parseFields`, `parseFactoryMethod`, `parseInitializers`, `parseEnum`,
//! `parseTypedef`, `parseImport`... do fasta), com os nós do `AstBuilder`.

use dartforge_frontend::token::{Keyword, Op};

use super::super::arvore::{No, NoId};
use super::super::token::{TipoToken, Tk};
use super::olhar::Forma;
use super::tipos::Membro;
use super::{Parser, R};

/// `DirectiveState` do fasta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Estado {
    Desconhecido,
    Script,
    Biblioteca,
    ImportExport,
    Parte,
    ParteDe,
    Declaracoes,
}

/// `DeclarationKind` dos membros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corpo {
    Classe,
    Mixin,
    Extensao,
    TipoExtensao,
    Enum,
}

/// Modificadores lidos (`ModifierContext`).
#[derive(Debug, Default, Clone, Copy)]
struct Mods {
    abstract_: Option<usize>,
    augment: Option<usize>,
    const_: Option<usize>,
    covariant: Option<usize>,
    external: Option<usize>,
    final_: Option<usize>,
    late: Option<usize>,
    required: Option<usize>,
    static_: Option<usize>,
    var: Option<usize>,
}

impl Mods {
    fn vfc(&self) -> Option<usize> {
        self.var.or(self.final_).or(self.const_)
    }
}

fn tk(o: Option<usize>) -> Option<Tk> {
    o.map(|i| i as Tk)
}

impl Parser<'_> {
    /// `parseUnit`.
    pub(super) fn unidade(&mut self) -> R<NoId> {
        let mut estado = Estado::Desconhecido;
        let mut script_tag = None;
        if self.tipo(0) == TipoToken::ScriptTag {
            script_tag = Some(0);
            estado = Estado::Script;
            self.pos = 1;
        }
        let mut directives = Vec::new();
        let mut declarations = Vec::new();
        while !self.eof(self.pos) {
            let (diretiva, id) = self.declaracao_topo(&mut estado)?;
            if diretiva {
                directives.push(id);
            } else {
                declarations.push(id);
            }
        }
        let eof = self.pos;
        Ok(self.no_entre(
            No::CompilationUnit {
                script_tag,
                directives,
                declarations,
            },
            0,
            eof,
        ))
    }

    fn palavra_de_topo(&self, i: usize) -> bool {
        self.kw(i, Keyword::Class)
            || self.kw(i, Keyword::Enum)
            || (self.ident(i)
                && matches!(
                    self.txt(i),
                    "export" | "extension" | "import" | "library" | "mixin" | "part" | "typedef"
                ))
    }

    fn marcar_declaracao(estado: &mut Estado) {
        if *estado != Estado::ParteDe {
            *estado = Estado::Declaracoes;
        }
    }

    /// `parseTopLevelDeclarationImpl`.
    fn declaracao_topo(&mut self, estado: &mut Estado) -> R<(bool, NoId)> {
        let ini = self.pos;
        let meta = self.metadados()?;
        let next = self.pos;
        if self.palavra_de_topo(next) {
            return self.declaracao_palavra(ini, meta, next, next, [None; 4], estado);
        }
        let comeco = next;
        let mut k = next;
        if self.modificador(next) {
            let seguinte_classe = |p: &Self, i: usize| p.kw(i, Keyword::Class) || p.pal(i, "mixin") || p.kw(i, Keyword::Enum);
            if self.kw(next, Keyword::Var)
                || self.pal(next, "late")
                || (self.kw(next, Keyword::Final) && !seguinte_classe(self, next + 1))
                || (self.kw(next, Keyword::Const) && !self.kw(next + 1, Keyword::Class))
            {
                Self::marcar_declaracao(estado);
                return Ok((false, self.membro_topo(ini, meta)?));
            }
            while self.modificador(k) {
                k += 1;
            }
        }
        let mut extras: [Option<usize>; 4] = [None; 4]; // macro, sealed, base, interface
        let mut inicio_mods = comeco;
        if self.pal(k, "macro") && self.kw(k + 1, Keyword::Class) {
            extras[0] = Some(k);
            k += 1;
        } else if self.pal(k, "sealed") {
            extras[1] = Some(k);
            if self.kw(k + 1, Keyword::Class) || self.pal(k + 1, "mixin") || self.kw(k + 1, Keyword::Enum) {
                k += 1;
            } else if self.pal(k + 1, "abstract") && self.kw(k + 2, Keyword::Class) {
                inicio_mods = k + 1;
                k += 2;
            }
        } else if self.pal(k, "base") {
            extras[2] = Some(k);
            if self.kw(k + 1, Keyword::Class) || self.pal(k + 1, "mixin") || self.kw(k + 1, Keyword::Enum) {
                k += 1;
            }
        } else if self.pal(k, "interface") {
            extras[3] = Some(k);
            if self.kw(k + 1, Keyword::Class) || self.pal(k + 1, "mixin") || self.kw(k + 1, Keyword::Enum) {
                k += 1;
            }
        }
        if self.palavra_de_topo(k) {
            return self.declaracao_palavra(ini, meta, inicio_mods, k, extras, estado);
        }
        if self.palavra(k) || k != comeco || self.op(k, Op::LParen) {
            Self::marcar_declaracao(estado);
            return Ok((false, self.membro_topo(ini, meta)?));
        }
        Err(self.erro_em(k, &format!("Expected a declaration, but got '{}'.", self.txt(k))))
    }

    /// Modificadores de `inicio` até `fim` (exclusivo) aceitos numa
    /// declaração de palavra-chave de topo.
    fn mods_de_topo(&mut self, inicio: usize, fim: usize, permitidos: &[&str]) -> R<Mods> {
        let salvo = self.pos;
        self.pos = inicio;
        let mut m = Mods::default();
        let r = self.coletar_modificadores(&mut m);
        let alcancado = self.pos;
        self.pos = salvo;
        r?;
        for i in inicio..alcancado {
            if !permitidos.contains(&self.txt(i)) {
                return Err(self.erro_em(i, &format!("Can't have modifier '{}' here.", self.txt(i))));
            }
        }
        if alcancado != fim {
            return Err(self.erro_em(alcancado, "Unexpected token."));
        }
        Ok(m)
    }

    /// `parseTopLevelKeywordDeclaration`.
    fn declaracao_palavra(
        &mut self,
        ini: usize,
        meta: Vec<NoId>,
        inicio_mods: usize,
        kw: usize,
        extras: [Option<usize>; 4],
        estado: &mut Estado,
    ) -> R<(bool, NoId)> {
        let [macro_, sealed, base, interface] = extras;
        // Os modificadores reconhecidos à parte ficam entre os comuns.
        let fim_mods = [macro_, sealed, base, interface]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(kw)
            .max(inicio_mods);
        let texto = self.txt(kw);
        if self.kw(kw, Keyword::Class) {
            Self::marcar_declaracao(estado);
            let m = self.mods_de_topo(inicio_mods, fim_mods, &["abstract", "augment", "final"])?;
            self.pos = kw;
            return Ok((false, self.classe(ini, meta, m, extras, None)?));
        }
        if self.kw(kw, Keyword::Enum) {
            Self::marcar_declaracao(estado);
            if base.is_some() || sealed.is_some() || interface.is_some() {
                return Err(self.erro_em(kw, "Enums can't be declared to be 'base', 'sealed' or 'interface'."));
            }
            let m = self.mods_de_topo(inicio_mods, fim_mods, &["augment"])?;
            self.pos = kw;
            return Ok((false, self.enumeracao(ini, meta, m)?));
        }
        let nv = kw + 1;
        let mut typedef_registro = false;
        if texto == "typedef" && self.op(nv, Op::LParen) {
            let f = self.par(nv) + 1;
            if self.ident(f) || (self.op(f, Op::Question) && self.ident(f + 1)) {
                let t = self.tipo_em(nv, false, false, false);
                if t.forma == Forma::Complexo && t.registro {
                    typedef_registro = true;
                }
            }
        }
        let membro = |p: &mut Self, estado: &mut Estado, meta: Vec<NoId>| -> R<(bool, NoId)> {
            Self::marcar_declaracao(estado);
            p.pos = inicio_mods;
            Ok((false, p.membro_topo(ini, meta)?))
        };
        if (self.op(nv, Op::LParen) || self.op(nv, Op::Dot)) && !typedef_registro {
            return membro(self, estado, meta);
        }
        if self.op(nv, Op::Lt) {
            if texto == "extension"
                && let Some(a) = self.args_tipo_lt(nv, true)
                && self.pal(a.ate, "on")
            {
                Self::marcar_declaracao(estado);
                let m = self.mods_de_topo(inicio_mods, kw, &["augment"])?;
                self.pos = kw;
                return Ok((false, self.extensao(ini, meta, m)?));
            }
            return membro(self, estado, meta);
        }
        if base.is_some() && texto != "mixin" || sealed.is_some() || interface.is_some() || macro_.is_some() {
            return Err(self.erro_em(kw, "Unexpected modifier."));
        }
        match texto {
            "import" | "export" | "part" | "library" => {
                self.mods_de_topo(inicio_mods, kw, &[])?;
                self.pos = kw;
                let d = match texto {
                    "import" => {
                        self.conferir_import_export(estado, kw)?;
                        self.importar(ini, meta)?
                    }
                    "export" => {
                        self.conferir_import_export(estado, kw)?;
                        self.exportar(ini, meta)?
                    }
                    "part" => self.parte(ini, meta, estado)?,
                    _ => {
                        if *estado < Estado::Biblioteca {
                            *estado = Estado::Biblioteca;
                        } else {
                            return Err(self.erro_em(kw, "The library directive must appear before all other directives."));
                        }
                        self.biblioteca(ini, meta)?
                    }
                };
                Ok((true, d))
            }
            "typedef" => {
                Self::marcar_declaracao(estado);
                let m = self.mods_de_topo(inicio_mods, kw, &["augment"])?;
                self.pos = kw;
                Ok((false, self.alias(ini, meta, m)?))
            }
            "mixin" => {
                if self.kw(nv, Keyword::Class) {
                    Self::marcar_declaracao(estado);
                    if sealed.is_some() || interface.is_some() {
                        return Err(self.erro_em(kw, "A mixin class can't be declared 'sealed' or 'interface'."));
                    }
                    let m = self.mods_de_topo(inicio_mods, fim_mods, &["abstract", "augment"])?;
                    self.pos = nv;
                    return Ok((false, self.classe(ini, meta, m, extras, Some(kw))?));
                }
                Self::marcar_declaracao(estado);
                let m = self.mods_de_topo(inicio_mods, fim_mods, &["augment"])?;
                self.pos = kw;
                Ok((false, self.mixin(ini, meta, m, base)?))
            }
            "extension" => {
                Self::marcar_declaracao(estado);
                let m = self.mods_de_topo(inicio_mods, kw, &["augment"])?;
                self.pos = kw;
                Ok((false, self.extensao(ini, meta, m)?))
            }
            _ => Err(self.erro_em(kw, "Unexpected token.")),
        }
    }

    fn conferir_import_export(&self, estado: &mut Estado, kw: usize) -> R<()> {
        match *estado {
            Estado::Desconhecido | Estado::Script | Estado::Biblioteca | Estado::ImportExport => {
                *estado = Estado::ImportExport;
                Ok(())
            }
            Estado::Parte => Err(self.erro_em(kw, "Import and export directives must come before part directives.")),
            Estado::ParteDe => Err(self.erro_em(kw, "The part-of directive must be the only directive in a part.")),
            Estado::Declaracoes => Err(self.erro_em(kw, "Directives must appear before any declarations.")),
        }
    }

    /// URI: um literal de string (`ensureLiteralString`).
    fn uri(&mut self) -> R<NoId> {
        if !self.texto(self.pos) {
            return Err(self.erro("Expected a string literal."));
        }
        self.literal_texto()
    }

    /// `parseLibraryName`.
    fn biblioteca(&mut self, ini: usize, metadata: Vec<NoId>) -> R<NoId> {
        let l = self.avancar();
        let name2 = if self.op(self.pos, Op::Semicolon) {
            None
        } else {
            Some(self.nome_qualificado(true)?)
        };
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::LibraryDirective {
                metadata,
                library_keyword: l as Tk,
                name2,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// `a.b.c` como [`No::LibraryIdentifier`] (ou [`No::DottedName`]).
    fn nome_qualificado(&mut self, biblioteca: bool) -> R<NoId> {
        let ini = self.pos;
        let mut components = Vec::new();
        loop {
            let i = self.espera_ident()?;
            components.push(self.simples(i));
            if !self.op(self.pos, Op::Dot) {
                break;
            }
            self.avancar();
        }
        let no = if biblioteca {
            No::LibraryIdentifier { components }
        } else {
            No::DottedName { components }
        };
        Ok(self.no(no, ini))
    }

    /// `parseConditionalUriStar`.
    fn configuracoes(&mut self, v: &mut Vec<NoId>) -> R<()> {
        while self.kw(self.pos, Keyword::If) {
            let i = self.avancar();
            let lp = self.espera_op(Op::LParen)?;
            let name = self.nome_qualificado(false)?;
            let (equal_token, value) = if self.op(self.pos, Op::EqEq) {
                let e = self.avancar();
                (Some(e as Tk), Some(self.uri()?))
            } else {
                (None, None)
            };
            let rp = self.espera_op(Op::RParen)?;
            let uri = self.uri()?;
            v.push(self.no(
                No::Configuration {
                    if_keyword: i as Tk,
                    left_parenthesis: lp as Tk,
                    name,
                    equal_token,
                    value,
                    right_parenthesis: rp as Tk,
                    uri,
                },
                i,
            ));
        }
        Ok(())
    }

    /// `parseCombinatorStar`.
    fn combinadores(&mut self, v: &mut Vec<NoId>) -> R<bool> {
        let mut algum = false;
        while self.pal(self.pos, "show") || self.pal(self.pos, "hide") {
            let k = self.avancar();
            let mut nomes = Vec::new();
            loop {
                let n = self.espera_ident()?;
                nomes.push(self.simples(n));
                if !self.op(self.pos, Op::Comma) {
                    break;
                }
                self.avancar();
            }
            let no = if self.txt(k) == "show" {
                No::ShowCombinator {
                    keyword: k as Tk,
                    shown_names: nomes,
                }
            } else {
                No::HideCombinator {
                    keyword: k as Tk,
                    hidden_names: nomes,
                }
            };
            v.push(self.no(no, k));
            algum = true;
        }
        Ok(algum)
    }

    /// `parseImport` com a recuperação de cláusulas fora de ordem
    /// (`parseImportRecovery`) nos casos que o fasta aceita sem erro.
    fn importar(&mut self, ini: usize, metadata: Vec<NoId>) -> R<NoId> {
        let i = self.avancar();
        if self.pal(self.pos, "augment") {
            return Err(self.erro("Augmentation imports aren't supported."));
        }
        let uri = self.uri()?;
        let mut configurations = Vec::new();
        let mut combinators = Vec::new();
        let mut deferred_keyword = None;
        let mut as_keyword = None;
        let mut prefix = None;
        let mut tem_combinador = false;
        loop {
            let inicio = self.pos;
            self.configuracoes(&mut configurations)?;
            if self.pal(self.pos, "deferred") && !self.pal(self.pos + 1, "as") {
                if deferred_keyword.is_some() {
                    return Err(self.erro("An import directive can only have one 'deferred' keyword."));
                }
                deferred_keyword = Some(self.avancar() as Tk);
            } else if self.pal(self.pos, "as") || (self.pal(self.pos, "deferred") && self.pal(self.pos + 1, "as")) {
                if self.pal(self.pos, "deferred") {
                    if deferred_keyword.is_some() {
                        return Err(self.erro("An import directive can only have one 'deferred' keyword."));
                    }
                    if as_keyword.is_some() {
                        return Err(self.erro("The deferred keyword should come immediately before the prefix ('as' clause)."));
                    }
                    deferred_keyword = Some(self.avancar() as Tk);
                }
                if as_keyword.is_some() {
                    return Err(self.erro("An import directive can only have one prefix ('as' clause)."));
                }
                if tem_combinador {
                    return Err(self.erro("The prefix ('as' clause) should come before any show/hide combinators."));
                }
                as_keyword = Some(self.avancar() as Tk);
                let n = self.espera_ident()?;
                prefix = Some(self.simples(n));
            }
            tem_combinador |= self.combinadores(&mut combinators)?;
            if self.op(self.pos, Op::Semicolon) {
                break;
            }
            if self.pos == inicio {
                return Err(self.esperado(";"));
            }
        }
        if deferred_keyword.is_some() && as_keyword.is_none() {
            return Err(self.erro("Deferred imports should have a prefix."));
        }
        let s = self.avancar();
        Ok(self.no(
            No::ImportDirective {
                metadata,
                import_keyword: i as Tk,
                uri,
                configurations,
                deferred_keyword,
                as_keyword,
                prefix,
                combinators,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// `parseExport`.
    fn exportar(&mut self, ini: usize, metadata: Vec<NoId>) -> R<NoId> {
        let e = self.avancar();
        let uri = self.uri()?;
        let mut configurations = Vec::new();
        self.configuracoes(&mut configurations)?;
        let mut combinators = Vec::new();
        self.combinadores(&mut combinators)?;
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::ExportDirective {
                metadata,
                export_keyword: e as Tk,
                uri,
                configurations,
                combinators,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// `parsePartOrPartOf`.
    fn parte(&mut self, ini: usize, metadata: Vec<NoId>, estado: &mut Estado) -> R<NoId> {
        let p = self.avancar();
        if self.pal(self.pos, "of") {
            if *estado != Estado::Desconhecido {
                return Err(self.erro_em(p, "The part-of directive must be the only directive in a part."));
            }
            *estado = Estado::ParteDe;
            let o = self.avancar();
            let (library_name, uri) = if self.ident(self.pos) {
                (Some(self.nome_qualificado(true)?), None)
            } else {
                (None, Some(self.uri()?))
            };
            let s = self.espera_op(Op::Semicolon)?;
            return Ok(self.no(
                No::PartOfDirective {
                    metadata,
                    part_keyword: p as Tk,
                    of_keyword: o as Tk,
                    library_name,
                    uri,
                    semicolon: s as Tk,
                },
                ini,
            ));
        }
        match *estado {
            Estado::ParteDe => return Err(self.erro_em(p, "The part-of directive must be the only directive in a part.")),
            Estado::Declaracoes => return Err(self.erro_em(p, "Directives must appear before any declarations.")),
            _ => *estado = Estado::Parte,
        }
        let uri = self.uri()?;
        if self.kw(self.pos, Keyword::If) {
            return Err(self.erro("Part directives can't have configurations."));
        }
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::PartDirective {
                metadata,
                part_keyword: p as Tk,
                uri,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// Port de `ModifierContext._parseModifiers`: lê modificadores em
    /// qualquer ordem, com os erros de ordem e conflito do fasta.
    fn coletar_modificadores(&mut self, m: &mut Mods) -> R<()> {
        while self.modificador(self.pos) {
            let i = self.pos;
            let fora = |p: &Self| Err(p.erro_em(i, &format!("The modifier '{}' is out of order.", p.txt(i))));
            let conflito = |p: &Self| Err(p.erro_em(i, &format!("Conflicting modifier '{}'.", p.txt(i))));
            match self.txt(i) {
                "abstract" => {
                    if m.abstract_.is_some() {
                        return conflito(self);
                    }
                    if m.vfc().is_some() || m.covariant.is_some() {
                        return fora(self);
                    }
                    m.abstract_ = Some(i);
                }
                "augment" => {
                    if m.augment.is_some() || m.external.is_some() {
                        return conflito(self);
                    }
                    if m.vfc().is_some() || m.abstract_.is_some() || m.covariant.is_some() || m.late.is_some() || m.static_.is_some() {
                        return fora(self);
                    }
                    m.augment = Some(i);
                }
                "const" => {
                    if m.vfc().is_some() || m.covariant.is_some() || m.late.is_some() {
                        return conflito(self);
                    }
                    m.const_ = Some(i);
                }
                "covariant" => {
                    if m.const_.is_some() || m.covariant.is_some() || m.static_.is_some() {
                        return conflito(self);
                    }
                    if m.var.is_some() || m.final_.is_some() || m.late.is_some() {
                        return fora(self);
                    }
                    m.covariant = Some(i);
                }
                "external" => {
                    if m.external.is_some() || m.augment.is_some() {
                        return conflito(self);
                    }
                    if m.const_.is_some() || m.static_.is_some() || m.late.is_some() || m.vfc().is_some() || m.covariant.is_some() {
                        return fora(self);
                    }
                    m.external = Some(i);
                }
                "final" => {
                    if m.vfc().is_some() {
                        return conflito(self);
                    }
                    m.final_ = Some(i);
                }
                "late" => {
                    if m.late.is_some() || m.const_.is_some() {
                        return conflito(self);
                    }
                    if m.var.is_some() || m.final_.is_some() {
                        return fora(self);
                    }
                    m.late = Some(i);
                }
                "required" => {
                    if m.required.is_some() {
                        return conflito(self);
                    }
                    if m.const_.is_some() || m.covariant.is_some() || m.final_.is_some() || m.var.is_some() {
                        return fora(self);
                    }
                    m.required = Some(i);
                }
                "static" => {
                    if m.covariant.is_some() || m.static_.is_some() {
                        return conflito(self);
                    }
                    if m.vfc().is_some() || m.late.is_some() {
                        return fora(self);
                    }
                    m.static_ = Some(i);
                }
                "var" => {
                    if m.vfc().is_some() {
                        return conflito(self);
                    }
                    m.var = Some(i);
                }
                _ => return Err(self.erro_em(i, "Unexpected modifier.")),
            }
            self.avancar();
        }
        Ok(())
    }

    /// `parseClassOrNamedMixinApplication`; `self.pos` está no `class`.
    fn classe(
        &mut self,
        ini: usize,
        metadata: Vec<NoId>,
        m: Mods,
        extras: [Option<usize>; 4],
        mixin: Option<usize>,
    ) -> R<NoId> {
        let [macro_, sealed, base, interface] = extras;
        let c = self.espera_kw(Keyword::Class)?;
        let name = self.espera_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.params_tipo()?)
        } else {
            None
        };
        if m.abstract_.is_some() {
            if sealed.is_some() {
                return Err(self.erro_em(sealed.unwrap_or(c), "A 'sealed' class can't be marked 'abstract' because it's already implicitly abstract."));
            }
            if m.final_.is_some() && (base.is_some() || interface.is_some()) {
                return Err(self.erro_em(c, "An 'abstract' class can't be declared as both 'final' and 'base' or 'interface'."));
            }
        }
        if self.op(self.pos, Op::Assign) {
            let equals = self.avancar();
            let superclass = self.tipo_nomeado_requerido()?;
            if !self.kw(self.pos, Keyword::With) {
                return Err(self.esperado("with"));
            }
            let with_clause = Some(self.clausula_lista(Keyword::With)?);
            let implements_clause = self.clausula_implements()?;
            let s = self.espera_op(Op::Semicolon)?;
            return Ok(self.no(
                No::ClassTypeAlias {
                    metadata,
                    augment_keyword: tk(m.augment),
                    abstract_keyword: tk(m.abstract_),
                    macro_keyword: tk(macro_),
                    sealed_keyword: tk(sealed),
                    base_keyword: tk(base),
                    interface_keyword: tk(interface),
                    final_keyword: tk(m.final_),
                    mixin_keyword: tk(mixin),
                    typedef_keyword: c as Tk,
                    name: name as Tk,
                    type_parameters,
                    equals: equals as Tk,
                    superclass,
                    with_clause,
                    implements_clause,
                    semicolon: s as Tk,
                },
                ini,
            ));
        }
        let extends_clause = if self.kw(self.pos, Keyword::Extends) {
            let e = self.avancar();
            let superclass = self.tipo_nomeado_requerido()?;
            if self.op(self.pos, Op::Comma) {
                return Err(self.erro("Each class definition can have at most one extends clause."));
            }
            Some(self.no(
                No::ExtendsClause {
                    extends_keyword: e as Tk,
                    superclass,
                },
                e,
            ))
        } else {
            None
        };
        let with_clause = if self.kw(self.pos, Keyword::With) {
            Some(self.clausula_lista(Keyword::With)?)
        } else {
            None
        };
        let implements_clause = self.clausula_implements()?;
        let native_clause = if self.pal(self.pos, "native") {
            let n = self.avancar();
            let name = if self.texto(self.pos) { Some(self.literal_texto()?) } else { None };
            Some(self.no(
                No::NativeClause {
                    native_keyword: n as Tk,
                    name,
                },
                n,
            ))
        } else {
            None
        };
        let nome = self.txt(name);
        let (lb, members, rb) = self.corpo_classe(Corpo::Classe, nome)?;
        Ok(self.no(
            No::ClassDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                abstract_keyword: tk(m.abstract_),
                macro_keyword: tk(macro_),
                sealed_keyword: tk(sealed),
                base_keyword: tk(base),
                interface_keyword: tk(interface),
                final_keyword: tk(m.final_),
                mixin_keyword: tk(mixin),
                class_keyword: c as Tk,
                name: name as Tk,
                type_parameters,
                extends_clause,
                with_clause,
                implements_clause,
                native_clause,
                left_bracket: lb as Tk,
                members,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// `with T, U` ([`No::WithClause`]).
    fn clausula_lista(&mut self, _k: Keyword) -> R<NoId> {
        let w = self.avancar();
        let mut tipos = Vec::new();
        loop {
            tipos.push(self.tipo_nomeado_requerido()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        Ok(self.no(
            No::WithClause {
                with_keyword: w as Tk,
                mixin_types: tipos,
            },
            w,
        ))
    }

    /// `implements T, U` opcional.
    fn clausula_implements(&mut self) -> R<Option<NoId>> {
        if !self.pal(self.pos, "implements") {
            return Ok(None);
        }
        let i = self.avancar();
        let mut tipos = Vec::new();
        loop {
            tipos.push(self.tipo_nomeado_requerido()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        Ok(Some(self.no(
            No::ImplementsClause {
                implements_keyword: i as Tk,
                interfaces: tipos,
            },
            i,
        )))
    }

    /// `{ membros }`.
    fn corpo_classe(&mut self, corpo: Corpo, nome: &str) -> R<(usize, Vec<NoId>, usize)> {
        if !self.op(self.pos, Op::LBrace) {
            return Err(self.esperado("{"));
        }
        let lb = self.avancar();
        let mut membros = Vec::new();
        while !self.op(self.pos, Op::RBrace) {
            if self.eof(self.pos) {
                return Err(self.esperado("}"));
            }
            membros.push(self.membro(corpo, nome)?);
        }
        let rb = self.avancar();
        Ok((lb, membros, rb))
    }

    /// `parseMixin`; `self.pos` está no `mixin`.
    fn mixin(&mut self, ini: usize, metadata: Vec<NoId>, m: Mods, base: Option<usize>) -> R<NoId> {
        if m.final_.is_some() {
            return Err(self.erro_em(m.final_.unwrap_or(ini), "A mixin can't be declared 'final'."));
        }
        let mk = self.avancar();
        let name = self.espera_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.params_tipo()?)
        } else {
            None
        };
        let on_clause = if self.pal(self.pos, "on") {
            let o = self.avancar();
            let mut tipos = Vec::new();
            loop {
                tipos.push(self.tipo_nomeado_requerido()?);
                if !self.op(self.pos, Op::Comma) {
                    break;
                }
                self.avancar();
            }
            Some(self.no(
                No::MixinOnClause {
                    on_keyword: o as Tk,
                    superclass_constraints: tipos,
                },
                o,
            ))
        } else {
            None
        };
        let implements_clause = self.clausula_implements()?;
        let nome = self.txt(name);
        let (lb, members, rb) = self.corpo_classe(Corpo::Mixin, nome)?;
        Ok(self.no(
            No::MixinDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                base_keyword: tk(base),
                mixin_keyword: mk as Tk,
                name: name as Tk,
                type_parameters,
                on_clause,
                implements_clause,
                left_bracket: lb as Tk,
                members,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// `parseExtension`; `self.pos` está no `extension`.
    fn extensao(&mut self, ini: usize, metadata: Vec<NoId>, m: Mods) -> R<NoId> {
        let e = self.avancar();
        if self.pal(self.pos, "type") && !self.pal(self.pos + 1, "on") && !self.op(self.pos + 1, Op::Lt) || self.pal(self.pos, "type") && self.kw(self.pos + 1, Keyword::Const) {
            return self.tipo_extensao(ini, metadata, m, e);
        }
        let name = if self.ident(self.pos) && !self.pal(self.pos, "on") {
            Some(self.avancar())
        } else {
            None
        };
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.params_tipo()?)
        } else {
            None
        };
        let o = self.espera_pal("on")?;
        let extended_type = self.tipo_requerido()?;
        let on_clause = Some(self.no(
            No::ExtensionOnClause {
                on_keyword: o as Tk,
                extended_type,
            },
            o,
        ));
        let nome = name.map_or("", |n| self.txt(n));
        let (lb, members, rb) = self.corpo_classe(Corpo::Extensao, nome)?;
        Ok(self.no(
            No::ExtensionDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                extension_keyword: e as Tk,
                name: tk(name),
                type_parameters,
                on_clause,
                left_bracket: lb as Tk,
                members,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// `parseExtensionTypeDeclaration`.
    fn tipo_extensao(&mut self, ini: usize, metadata: Vec<NoId>, m: Mods, e: usize) -> R<NoId> {
        let t = self.avancar();
        let const_keyword = if self.kw(self.pos, Keyword::Const) {
            Some(self.avancar() as Tk)
        } else {
            None
        };
        let name = self.espera_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.params_tipo()?)
        } else {
            None
        };
        let ri = self.pos;
        let constructor_name = if self.op(self.pos, Op::Dot) {
            let p = self.avancar();
            if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                return Err(self.erro("Expected an identifier."));
            }
            let n = self.avancar();
            Some(self.no(
                No::RepresentationConstructorName {
                    period: p as Tk,
                    name: n as Tk,
                },
                p,
            ))
        } else {
            None
        };
        let lp = self.espera_op(Op::LParen)?;
        let field_metadata = self.metadados()?;
        if self.modificador(self.pos) {
            return Err(self.erro("Representation fields can't have modifiers."));
        }
        let ti = self.tipo_em(self.pos, false, false, true);
        if ti.e_nenhum() {
            return Err(self.erro("Expected a representation type."));
        }
        let field_type = self.tipo_ate(ti.ate)?;
        let field_name = self.espera_ident()?;
        self.opcional_op(Op::Comma);
        let rp = self.espera_op(Op::RParen)?;
        let representation = self.no(
            No::RepresentationDeclaration {
                constructor_name,
                left_parenthesis: lp as Tk,
                field_metadata,
                field_type,
                field_name: field_name as Tk,
                right_parenthesis: rp as Tk,
            },
            ri,
        );
        let implements_clause = self.clausula_implements()?;
        let nome = self.txt(name);
        let (lb, members, rb) = self.corpo_classe(Corpo::TipoExtensao, nome)?;
        Ok(self.no(
            No::ExtensionTypeDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                extension_keyword: e as Tk,
                type_keyword: t as Tk,
                const_keyword,
                name: name as Tk,
                type_parameters,
                representation,
                implements_clause,
                left_bracket: lb as Tk,
                members,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// `parseEnum`; `self.pos` está no `enum`.
    fn enumeracao(&mut self, ini: usize, metadata: Vec<NoId>, m: Mods) -> R<NoId> {
        let e = self.avancar();
        let name = self.espera_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.params_tipo()?)
        } else {
            None
        };
        let with_clause = if self.kw(self.pos, Keyword::With) {
            Some(self.clausula_lista(Keyword::With)?)
        } else {
            None
        };
        let implements_clause = self.clausula_implements()?;
        let lb = self.espera_op(Op::LBrace)?;
        let mut constants = Vec::new();
        loop {
            if self.op(self.pos, Op::RBrace) || self.op(self.pos, Op::Semicolon) {
                break;
            }
            constants.push(self.constante_enum()?);
            if self.op(self.pos, Op::Comma) {
                self.avancar();
            } else if self.op(self.pos, Op::RBrace) || self.op(self.pos, Op::Semicolon) {
                break;
            } else {
                return Err(self.esperado("}"));
            }
        }
        let mut semicolon = None;
        let mut members = Vec::new();
        if self.op(self.pos, Op::Semicolon) {
            semicolon = Some(self.avancar() as Tk);
            let nome = self.txt(name);
            while !self.op(self.pos, Op::RBrace) {
                if self.eof(self.pos) {
                    return Err(self.esperado("}"));
                }
                members.push(self.membro(Corpo::Enum, nome)?);
            }
        }
        let rb = self.espera_op(Op::RBrace)?;
        Ok(self.no(
            No::EnumDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                enum_keyword: e as Tk,
                name: name as Tk,
                type_parameters,
                with_clause,
                implements_clause,
                left_bracket: lb as Tk,
                constants,
                semicolon,
                members,
                right_bracket: rb as Tk,
            },
            ini,
        ))
    }

    /// `parseEnumElement`.
    fn constante_enum(&mut self) -> R<NoId> {
        let ini = self.pos;
        let metadata = self.metadados()?;
        if self.pal(self.pos, "augment") && self.ident(self.pos + 1) {
            return Err(self.erro("Augmentations aren't supported here."));
        }
        let name = self.espera_ident()?;
        let ai = self.pos;
        let type_arguments = if self.op(self.pos, Op::Lt) {
            if self.args_tipo_em(name, false).is_none() {
                return Err(self.erro("Invalid type arguments."));
            }
            Some(self.args_tipo()?)
        } else {
            None
        };
        let constructor_selector = if self.op(self.pos, Op::Dot) {
            let p = self.avancar();
            if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                return Err(self.erro("Expected an identifier."));
            }
            let n = self.avancar();
            let s = self.simples(n);
            Some(self.no(
                No::ConstructorSelector {
                    period: p as Tk,
                    name: s,
                },
                p,
            ))
        } else {
            None
        };
        let arguments = if self.op(self.pos, Op::LParen) || type_arguments.is_some() || constructor_selector.is_some() {
            if !self.op(self.pos, Op::LParen) {
                return Err(self.esperado("("));
            }
            let argument_list = self.argumentos()?;
            Some(self.no(
                No::EnumConstantArguments {
                    type_arguments,
                    constructor_selector,
                    argument_list,
                },
                ai,
            ))
        } else {
            None
        };
        Ok(self.no(
            No::EnumConstantDeclaration {
                metadata,
                name: name as Tk,
                arguments,
            },
            ini,
        ))
    }

    /// `parseTypedef`; `self.pos` está no `typedef`.
    fn alias(&mut self, ini: usize, metadata: Vec<NoId>, m: Mods) -> R<NoId> {
        let td = self.avancar();
        let ti = self.tipo_em(self.pos, false, false, false);
        let nx = ti.ate;
        let depois = self.args_tipo_em(nx, true).map_or(nx + 1, |a| a.ate);
        if ti.e_nenhum() && self.op(depois, Op::Assign) {
            if !self.ident(self.pos) {
                return Err(self.erro("Expected an identifier."));
            }
            let name = self.avancar();
            let type_parameters = if self.op(self.pos, Op::Lt) {
                Some(self.params_tipo()?)
            } else {
                None
            };
            let equals = self.espera_op(Op::Assign)?;
            let type_ = self.tipo_requerido()?;
            let s = self.espera_op(Op::Semicolon)?;
            return Ok(self.no(
                No::GenericTypeAlias {
                    metadata,
                    augment_keyword: tk(m.augment),
                    typedef_keyword: td as Tk,
                    name: name as Tk,
                    type_parameters,
                    equals: equals as Tk,
                    type_,
                    semicolon: s as Tk,
                },
                ini,
            ));
        }
        let return_type = if ti.e_nenhum() {
            None
        } else {
            Some(self.tipo_ate(ti.ate)?)
        };
        let name = self.espera_ident()?;
        let type_parameters = if self.op(self.pos, Op::Lt) {
            Some(self.params_tipo()?)
        } else {
            None
        };
        if !self.op(self.pos, Op::LParen) {
            return Err(self.esperado("("));
        }
        let parameters = self.params_formais(Membro::AliasFuncao)?;
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::FunctionTypeAlias {
                metadata,
                augment_keyword: tk(m.augment),
                typedef_keyword: td as Tk,
                return_type,
                name: name as Tk,
                type_parameters,
                parameters,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// `indicatesMethodOrField`.
    fn indica_metodo_ou_campo(&self, i: usize) -> bool {
        matches!(
            self.tipo(i),
            TipoToken::Op(Op::Semicolon | Op::Assign | Op::LParen | Op::LBrace | Op::Arrow | Op::Lt)
        )
    }

    /// `parseTopLevelMemberImpl`.
    fn membro_topo(&mut self, ini: usize, metadata: Vec<NoId>) -> R<NoId> {
        let mut m = Mods::default();
        self.coletar_modificadores(&mut m)?;
        for (x, nome) in [(m.abstract_, "abstract"), (m.covariant, "covariant"), (m.required, "required"), (m.static_, "static")] {
            if let Some(i) = x {
                return Err(self.erro_em(i, &format!("Can't have modifier '{nome}' here.")));
            }
        }
        if m.vfc().is_some()
            && let Some(f) = self.pular_padrao_externo(self.pos)
            && self.op(f + 1, Op::Assign)
        {
            return Err(self.erro("Pattern variable declarations aren't allowed outside of functions or methods."));
        }
        let ti = self.tipo_em(self.pos, false, true, false);
        let mut nx = ti.ate;
        let mut prop = None;
        if (self.pal(nx, "get") || self.pal(nx, "set")) && self.ident(nx + 1) {
            prop = Some(nx);
            nx += 1;
        }
        if ti.e_nenhum() && m.vfc().is_none() && self.reservada(nx + 1) && self.indica_metodo_ou_campo(nx + 2) {
            return Err(self.erro_em(nx + 1, "Expected an identifier."));
        }
        if !self.ident_puro(nx) {
            let v = self.txt(nx);
            if v == "factory" || v == "operator" {
                if prop.is_none() && !self.indica_metodo_ou_campo(nx + 1) && !self.op(nx + 1, Op::Comma) {
                    return Err(self.erro_em(nx, "Top-level declarations can't be factories or operators."));
                }
            } else if !self.ident(nx) {
                return Err(self.erro_em(nx, &format!("Expected an identifier, but got '{}'.", self.txt(nx))));
            }
        }
        let depois = nx + 1;
        if prop.is_some()
            || matches!(
                self.tipo(depois),
                TipoToken::Op(Op::LParen | Op::LBrace | Op::Lt | Op::Dot | Op::Arrow)
            )
        {
            if let Some(v) = m.vfc().or(m.late) {
                return Err(self.erro_em(v, "Functions can't be declared 'var', 'final', 'const' or 'late'."));
            }
            let return_type = if ti.e_nenhum() {
                None
            } else {
                Some(self.tipo_ate(ti.ate)?)
            };
            if prop.is_some() {
                self.avancar();
            }
            let name = self.espera_ident()?;
            let getter = prop.is_some_and(|p| self.txt(p) == "get");
            let fi = self.pos;
            let type_parameters = if prop.is_none() && self.op(self.pos, Op::Lt) {
                Some(self.params_tipo()?)
            } else {
                None
            };
            let parameters = if self.op(self.pos, Op::LParen) {
                if getter {
                    return Err(self.erro("Getters must be declared without a parameter list."));
                }
                Some(self.params_formais(Membro::FuncaoTopo)?)
            } else if getter {
                None
            } else {
                return Err(self.erro("Functions must have an explicit list of parameters."));
            };
            if m.external.is_some() && !self.op(self.pos, Op::Semicolon) {
                return Err(self.erro("An external or native method can't have a body."));
            }
            let body = self.corpo_funcao(false, m.external.is_some())?;
            let fe = self.no(
                No::FunctionExpression {
                    type_parameters,
                    parameters,
                    body,
                },
                fi,
            );
            return Ok(self.no(
                No::FunctionDeclaration {
                    metadata,
                    augment_keyword: tk(m.augment),
                    external_keyword: tk(m.external),
                    return_type,
                    property_keyword: tk(prop),
                    name: name as Tk,
                    function_expression: fe,
                },
                ini,
            ));
        }
        if prop.is_some() {
            return Err(self.erro_em(nx, "Unexpected 'get'/'set'."));
        }
        let fields = self.campos(&m, ti, None)?;
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::TopLevelVariableDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                external_keyword: tk(m.external),
                variables: fields,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// `parseFields` até antes do `;`: o [`No::VariableDeclarationList`].
    fn campos(&mut self, m: &Mods, ti: super::olhar::InfoTipo, nome_classe: Option<&str>) -> R<NoId> {
        if let Some(c) = m.covariant
            && m.late.is_none()
            && m.final_.is_some()
        {
            return Err(self.erro_em(c, "Members can't be declared to be both 'final' and 'covariant'."));
        }
        if ti.e_nenhum() {
            if m.vfc().is_none() {
                return Err(self.erro_em(ti.ate, "Variables must be declared using the keywords 'const', 'final', 'var' or a type name."));
            }
        } else if let Some(v) = m.var {
            return Err(self.erro_em(v, "Variables can't be declared using both 'var' and a type name."));
        }
        if let (Some(a), Some(_)) = (m.abstract_, m.external) {
            return Err(self.erro_em(a, "Fields can't be declared both 'abstract' and 'external'."));
        }
        let li = self.pos;
        let type_ = if ti.e_nenhum() {
            None
        } else {
            Some(self.tipo_ate(ti.ate)?)
        };
        let mut vars = Vec::new();
        loop {
            let n = self.espera_ident()?;
            if nome_classe.is_some_and(|c| c == self.txt(n)) {
                return Err(self.erro_em(n, "A class member can't have the same name as the enclosing class."));
            }
            let (equals, initializer) = if self.op(self.pos, Op::Assign) {
                let e = self.avancar();
                (Some(e as Tk), Some(self.expressao()?))
            } else {
                (None, None)
            };
            vars.push(self.no(
                No::VariableDeclaration {
                    name: n as Tk,
                    equals,
                    initializer,
                },
                n,
            ));
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        let ini = [m.late, m.vfc()].into_iter().flatten().min().unwrap_or(li).min(li);
        Ok(self.no(
            No::VariableDeclarationList {
                metadata: Vec::new(),
                late_keyword: tk(m.late),
                keyword: tk(m.vfc()),
                type_,
                variables: vars,
            },
            ini,
        ))
    }

    /// `parseClassOrMixinOrExtensionOrEnumMemberImpl`.
    fn membro(&mut self, corpo: Corpo, nome_classe: &str) -> R<NoId> {
        let ini = self.pos;
        let metadata = self.metadados()?;
        let mut m = Mods::default();
        self.coletar_modificadores(&mut m)?;
        if let Some(r) = m.required {
            return Err(self.erro_em(r, "Can't have modifier 'required' here."));
        }
        if m.vfc().is_some()
            && let Some(f) = self.pular_padrao_externo(self.pos)
            && self.op(f + 1, Op::Assign)
        {
            return Err(self.erro("Pattern variable declarations aren't allowed outside of functions or methods."));
        }
        let ti = self.tipo_em(self.pos, false, true, false);
        let mut nx = ti.ate;
        let mut get_set = None;
        if !self.ident_puro(nx) {
            let v = self.txt(nx);
            if (v == "get" || v == "set") && self.ident(nx) {
                if self.ident(nx + 1) {
                    get_set = Some(nx);
                    nx += 1;
                } else if self.reservada(nx + 1) && self.indica_metodo_ou_campo(nx + 2) {
                    return Err(self.erro_em(nx + 1, "Expected an identifier."));
                }
            } else if v == "factory" && self.ident(nx) {
                let n2 = nx + 1;
                if self.ident(n2) || self.modificador(n2) {
                    if !ti.e_nenhum() {
                        return Err(self.erro_em(nx, "Factory constructors cannot have a return type."));
                    }
                    if m.abstract_.is_some() {
                        return Err(self.erro("Members of classes can't be declared to be 'abstract'."));
                    }
                    return self.fabrica(ini, metadata, m, nx, corpo);
                }
            } else if v == "operator" && self.ident(nx) {
                let n2 = nx + 1;
                if self.operador_definivel(n2).is_some() && self.args_tipo_em(nx, false).is_none() {
                    return self.metodo(ini, metadata, m, ti, None, nx, corpo, nome_classe);
                } else if !self.op(n2, Op::Assign) && !self.op(n2, Op::Lt) && !self.palavra(n2) && !self.op(n2, Op::LParen) && !self.op(n2, Op::Semicolon) {
                    return Err(self.erro_em(n2, "Invalid operator."));
                }
            } else if !self.ident(nx) || (v == "typedef" && ti.e_nenhum() && ini == self.pos && self.ident(nx + 1)) {
                return Err(self.erro_em(nx, &format!("Expected a class member, but got '{}'.", self.txt(nx))));
            }
        } else if ti.e_nenhum() && m.vfc().is_none() {
            let n2 = nx + 1;
            if self.operador_definivel(n2).is_some()
                && !self.op(n2, Op::LBracket)
                && (self.op(n2 + 1, Op::LParen) || self.op(n2 + 1, Op::LBrace) || self.op(n2 + 1, Op::Arrow))
            {
                return Err(self.erro_em(n2, "Operator declarations must be preceded by the keyword 'operator'."));
            } else if self.reservada(n2) && self.indica_metodo_ou_campo(n2 + 1) {
                return Err(self.erro_em(n2, "Expected an identifier."));
            }
        }
        let depois = nx + 1;
        if get_set.is_some()
            || matches!(
                self.tipo(depois),
                TipoToken::Op(Op::LParen | Op::LBrace | Op::Lt | Op::Dot | Op::Arrow)
            )
        {
            return self.metodo(ini, metadata, m, ti, get_set, nx, corpo, nome_classe);
        }
        if get_set.is_some() {
            return Err(self.erro("Unexpected 'get'/'set'."));
        }
        match corpo {
            Corpo::Extensao => {
                if m.abstract_.is_some() {
                    return Err(self.erro("Extensions can't declare abstract members."));
                }
                if m.static_.is_none() && m.external.is_none() {
                    return Err(self.erro_em(nx, "Extensions can't declare instance fields."));
                }
            }
            _ => {}
        }
        let fields = self.campos(&m, ti, Some(nome_classe))?;
        let s = self.espera_op(Op::Semicolon)?;
        Ok(self.no(
            No::FieldDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                abstract_keyword: tk(m.abstract_),
                covariant_keyword: tk(m.covariant),
                external_keyword: tk(m.external),
                static_keyword: tk(m.static_),
                fields,
                semicolon: s as Tk,
            },
            ini,
        ))
    }

    /// `parseMethod`: métodos, getters, setters, operadores e construtores
    /// não-fábrica. `nome` é o índice do nome (ou do `operator`).
    #[allow(clippy::too_many_arguments)]
    fn metodo(
        &mut self,
        ini: usize,
        metadata: Vec<NoId>,
        m: Mods,
        ti: super::olhar::InfoTipo,
        get_set: Option<usize>,
        nome: usize,
        corpo: Corpo,
        nome_classe: &str,
    ) -> R<NoId> {
        if let Some(a) = m.abstract_ {
            return Err(self.erro_em(a, "Members of classes can't be declared to be 'abstract'."));
        }
        if let Some(l) = m.late {
            return Err(self.erro_em(l, "Can't have modifier 'late' here."));
        }
        let operador = get_set.is_none()
            && self.pal(nome, "operator")
            && self.operador_definivel(nome + 1).is_some()
            && self.args_tipo_em(nome, false).is_none();
        if m.static_.is_some() && operador {
            return Err(self.erro("Operators can't be static."));
        }
        if m.static_.is_none()
            && let Some(c) = m.covariant
            && get_set.is_none_or(|g| self.txt(g) == "get")
        {
            return Err(self.erro_em(c, "Getters, setters and methods can't be declared to be 'covariant'."));
        }
        if let Some(v) = m.var.or(m.final_) {
            return Err(self.erro_em(v, "Methods can't be declared 'var' or 'final'."));
        }
        if m.const_.is_some() && get_set.is_some() {
            return Err(self.erro("Getters and setters can't be 'const'."));
        }
        let return_type = if ti.e_nenhum() {
            None
        } else {
            Some(self.tipo_ate(ti.ate)?)
        };
        if get_set.is_some() {
            self.avancar();
        }
        let mut operator_keyword = None;
        let mut period = None;
        let mut nome2 = None;
        let name = if operador {
            operator_keyword = Some(self.avancar() as Tk);
            self.consumir_operador_definivel()?
        } else {
            let n = self.espera_ident()?;
            if self.op(self.pos, Op::Dot) {
                period = Some(self.avancar());
                if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                    return Err(self.erro("Expected an identifier."));
                }
                nome2 = Some(self.avancar());
            }
            n
        };
        let fi = self.pos;
        let getter = get_set.is_some_and(|g| self.txt(g) == "get") && period.is_none() && !self.op(self.pos, Op::Colon);
        let type_parameters = if get_set.is_none() && self.op(self.pos, Op::Lt) {
            Some(self.params_tipo()?)
        } else {
            None
        };
        let membro = match (corpo, m.static_.is_some()) {
            (Corpo::Extensao | Corpo::TipoExtensao, _) => Membro::Extensao,
            (_, true) => Membro::MetodoEstatico,
            _ => Membro::MetodoInstancia,
        };
        let parameters = if self.op(self.pos, Op::LParen) {
            if getter {
                return Err(self.erro("Getters must be declared without a parameter list."));
            }
            Some(self.params_formais(membro)?)
        } else if getter {
            None
        } else {
            return Err(self.erro("Methods must have an explicit list of parameters."));
        };
        let mut separator = None;
        let mut initializers = Vec::new();
        if self.op(self.pos, Op::Colon) {
            separator = Some(self.avancar() as Tk);
            initializers = self.inicializadores()?;
        }
        let corpo_no = self.pos;
        if m.external.is_some() && !self.op(self.pos, Op::Semicolon) && !self.pal(self.pos, "async") && !self.pal(self.pos, "sync") {
            return Err(self.erro("An external or native method can't have a body."));
        }
        if self.op(self.pos, Op::Assign) {
            return Err(self.erro("Only factory constructor can specify '=' redirection."));
        }
        let body = self.corpo_funcao(false, m.static_.is_none() || m.external.is_some())?;
        let construtor = period.is_some() || separator.is_some() || (self.txt(name) == nome_classe && !operador);
        if construtor && get_set.is_none() {
            if let Some(s) = m.static_ {
                return Err(self.erro_em(s, "Constructors can't be static."));
            }
            if !ti.e_nenhum() {
                return Err(self.erro_em(self.ini(return_type.unwrap_or(NoId(0))), "Constructors can't have a return type."));
            }
            if self.txt(name) != nome_classe {
                return Err(self.erro_em(name, "The name of a constructor must match the name of the enclosing class."));
            }
            if separator.is_some() && m.external.is_some() {
                return Err(self.erro("An external constructor can't have any initializers."));
            }
            if type_parameters.is_some() {
                return Err(self.erro_em(fi, "Constructors can't have type parameters."));
            }
            if matches!(corpo, Corpo::Mixin | Corpo::Extensao) {
                return Err(self.erro_em(name, "Mixins and extensions can't declare constructors."));
            }
            if matches!(self.arv.no(body), No::BlockFunctionBody { .. } | No::ExpressionFunctionBody { .. })
                && initializers.iter().any(|&i| matches!(self.arv.no(i), No::RedirectingConstructorInvocation { .. }))
            {
                return Err(self.erro_em(corpo_no, "Redirecting constructors can't have a body."));
            }
            let return_type = self.simples(name);
            return Ok(self.no(
                No::ConstructorDeclaration {
                    metadata,
                    augment_keyword: tk(m.augment),
                    external_keyword: tk(m.external),
                    const_keyword: tk(m.const_),
                    factory_keyword: None,
                    return_type,
                    period: tk(period),
                    name: tk(nome2),
                    parameters: parameters.unwrap_or(NoId(0)),
                    separator,
                    initializers,
                    redirected_constructor: None,
                    body,
                },
                ini,
            ));
        }
        if construtor {
            return Err(self.erro_em(name, "A class member can't have the same name as the enclosing class."));
        }
        if m.const_.is_some() {
            return Err(self.erro("Methods can't be declared 'const'."));
        }
        if corpo == Corpo::Extensao
            && matches!(self.arv.no(body), No::EmptyFunctionBody { .. })
            && m.external.is_none()
        {
            return Err(self.erro_em(name, "Extensions can't declare abstract members."));
        }
        Ok(self.no(
            No::MethodDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                external_keyword: tk(m.external),
                modifier_keyword: tk(m.static_),
                return_type,
                property_keyword: tk(get_set),
                operator_keyword,
                name: name as Tk,
                type_parameters,
                parameters,
                body,
            },
            ini,
        ))
    }

    /// `parseFactoryMethod`; `f` é o índice do `factory`.
    fn fabrica(&mut self, ini: usize, metadata: Vec<NoId>, m: Mods, f: usize, corpo: Corpo) -> R<NoId> {
        self.pos = f + 1;
        if !self.ref_tipo_valida(self.pos) {
            return Err(self.erro("Modifiers must come before 'factory'."));
        }
        if let Some(x) = m.static_.or(m.covariant) {
            return Err(self.erro_em(x, "Can't have this modifier on a factory."));
        }
        if let Some(v) = m.var.or(m.final_) {
            return Err(self.erro_em(v, "Can't have this modifier on a factory."));
        }
        if matches!(corpo, Corpo::Mixin | Corpo::Extensao) {
            return Err(self.erro_em(f, "Mixins and extensions can't declare constructors."));
        }
        let name = self.espera_ident()?;
        let (period, nome2) = if self.op(self.pos, Op::Dot) {
            let p = self.avancar();
            if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                return Err(self.erro("Expected an identifier."));
            }
            (Some(p), Some(self.avancar()))
        } else {
            (None, None)
        };
        if self.op(self.pos, Op::Lt) {
            return Err(self.erro("Constructors can't have type parameters."));
        }
        if !self.op(self.pos, Op::LParen) {
            return Err(self.esperado("("));
        }
        let parameters = self.params_formais(Membro::Fabrica)?;
        let mut separator = None;
        let mut redirected_constructor = None;
        let body = if self.op(self.pos, Op::Assign) {
            if m.external.is_some() {
                return Err(self.erro("A redirecting factory can't be external."));
            }
            separator = Some(self.avancar() as Tk);
            redirected_constructor = Some(self.nome_construtor()?);
            let s = self.espera_op(Op::Semicolon)?;
            self.no(No::EmptyFunctionBody { semicolon: s as Tk }, s)
        } else if m.external.is_some() {
            if !self.op(self.pos, Op::Semicolon) {
                return Err(self.erro("External factories can't have a body."));
            }
            self.corpo_funcao(false, true)?
        } else {
            self.corpo_funcao(false, false)?
        };
        let return_type = self.simples(name);
        Ok(self.no(
            No::ConstructorDeclaration {
                metadata,
                augment_keyword: tk(m.augment),
                external_keyword: tk(m.external),
                const_keyword: tk(m.const_),
                factory_keyword: Some(f as Tk),
                return_type,
                period: tk(period),
                name: tk(nome2),
                parameters,
                separator,
                initializers: Vec::new(),
                redirected_constructor,
                body,
            },
            ini,
        ))
    }

    /// `parseInitializers` (depois do `:`).
    fn inicializadores(&mut self) -> R<Vec<NoId>> {
        let old = self.funcoes;
        self.funcoes = false;
        let mut v = Vec::new();
        loop {
            v.push(self.inicializador()?);
            if !self.op(self.pos, Op::Comma) {
                break;
            }
            self.avancar();
        }
        self.funcoes = old;
        Ok(v)
    }

    /// O lado direito de `campo = ...` (o resto de `parseExpression` depois
    /// do `=` de uma atribuição).
    fn direita_inicializador(&mut self) -> R<NoId> {
        if self.kw(self.pos, Keyword::Throw) {
            return self.expressao_sem_cascata();
        }
        self.entrar()?;
        let r = self.precedencia(super::prec::ATRIBUICAO, true, super::expressoes::CtxConst::Nenhum);
        self.sair();
        r
    }

    /// `parseInitializer`.
    fn inicializador(&mut self) -> R<NoId> {
        let i = self.pos;
        if self.kw(i, Keyword::Assert) {
            let (a, lp, condition, comma, message, rp) = self.partes_assert()?;
            return Ok(self.no(
                No::AssertInitializer {
                    assert_keyword: a as Tk,
                    left_parenthesis: lp as Tk,
                    condition,
                    comma: tk(comma),
                    message,
                    right_parenthesis: rp as Tk,
                },
                a,
            ));
        }
        if self.kw(i, Keyword::Super) {
            self.avancar();
            let (period, constructor_name) = if self.op(self.pos, Op::Dot) {
                let p = self.avancar();
                if !(self.ident(self.pos) || self.kw(self.pos, Keyword::New)) {
                    return Err(self.erro("Expected an identifier."));
                }
                let n = self.avancar();
                (Some(p as Tk), Some(self.simples(n)))
            } else {
                (None, None)
            };
            if !self.op(self.pos, Op::LParen) {
                return Err(self.esperado("("));
            }
            let argument_list = self.argumentos()?;
            return Ok(self.no(
                No::SuperConstructorInvocation {
                    super_keyword: i as Tk,
                    period,
                    constructor_name,
                    argument_list,
                },
                i,
            ));
        }
        if self.kw(i, Keyword::This) {
            self.avancar();
            let mut period = None;
            let mut nome = None;
            if self.op(self.pos, Op::Dot) {
                period = Some(self.avancar());
                if !(self.ident(self.pos) || (self.kw(self.pos, Keyword::New) && self.op(self.pos + 1, Op::LParen))) {
                    return Err(self.erro("Expected an identifier."));
                }
                nome = Some(self.avancar());
                if self.op(self.pos, Op::Assign) {
                    let field_name = self.simples(nome.unwrap_or(0));
                    let eq = self.avancar();
                    let expression = self.direita_inicializador()?;
                    return Ok(self.no(
                        No::ConstructorFieldInitializer {
                            this_keyword: Some(i as Tk),
                            period: tk(period),
                            field_name,
                            equals: eq as Tk,
                            expression,
                        },
                        i,
                    ));
                }
            }
            if self.op(self.pos, Op::LParen) {
                let constructor_name = nome.map(|n| self.simples(n));
                let argument_list = self.argumentos()?;
                return Ok(self.no(
                    No::RedirectingConstructorInvocation {
                        this_keyword: i as Tk,
                        period: tk(period),
                        constructor_name,
                        argument_list,
                    },
                    i,
                ));
            }
            return Err(self.esperado("."));
        }
        if self.ident(i) && self.op(i + 1, Op::Assign) {
            self.avancar();
            let field_name = self.simples(i);
            let eq = self.avancar();
            let expression = self.direita_inicializador()?;
            return Ok(self.no(
                No::ConstructorFieldInitializer {
                    this_keyword: None,
                    period: None,
                    field_name,
                    equals: eq as Tk,
                    expression,
                },
                i,
            ));
        }
        Err(self.erro("Expected an initializer."))
    }
}
