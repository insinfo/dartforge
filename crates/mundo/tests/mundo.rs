//! O mundo fechado sobre um programa real (`tests/programas/casos.dart`),
//! com asserções pelo nome do elemento. Compila contra o SDK: `--ignored`.

use dartforge_elements::model::{ClassId, Element, FunctionElementId, Program, VariableId};
use dartforge_intern::Interner;
use dartforge_mundo::{Entrada, Mundo, NivelClasse, Raizes, calcular, conferir};
use std::path::Path;

struct Caso<'a> {
    p: &'a Program,
    i: &'a Interner,
    m: &'a Mundo,
}

impl Caso<'_> {
    fn classe(&self, nome: &str) -> ClassId {
        (0..self.p.classes.len())
            .map(|i| ClassId(i as u32))
            .find(|c| !self.p.library(self.p.class(*c).library).is_sdk && self.i.resolve(self.p.class(*c).name) == nome)
            .unwrap_or_else(|| panic!("classe {nome}"))
    }
    /// `C.m` (membro de classe), `ext:m` (membro de extensão) ou `m` (topo).
    fn funcao(&self, nome: &str) -> FunctionElementId {
        let (classe, membro) = match nome.split_once('.') {
            Some((c, m)) => (Some(self.classe(c)), m),
            None => (None, nome),
        };
        let ext = membro.strip_prefix("ext:");
        let membro = ext.unwrap_or(membro);
        (0..self.p.functions.len())
            .map(|i| FunctionElementId(i as u32))
            .find(|f| {
                let func = self.p.function(*f);
                !self.p.library(func.library).is_sdk
                    && self.i.resolve(func.name) == membro
                    && func.class == classe
                    && func.extension.is_some() == ext.is_some()
            })
            .unwrap_or_else(|| panic!("função {nome}"))
    }
    fn variavel(&self, nome: &str) -> VariableId {
        (0..self.p.variables.len())
            .map(|i| VariableId(i as u32))
            .find(|v| !self.p.library(self.p.variable(*v).library).is_sdk && self.i.resolve(self.p.variable(*v).name) == nome)
            .unwrap_or_else(|| panic!("variável {nome}"))
    }
    fn viva(&self, nome: &str) -> bool {
        self.m.funcao(self.funcao(nome))
    }
    /// `C.m` por espécie: `escrita` escolhe o setter (`set m`), senão o
    /// getter/método. Distingue pelo `FunctionKind`, porque os dois se chamam `m`.
    fn viva_especie(&self, nome: &str, escrita: bool) -> bool {
        use dartforge_elements::model::FunctionKind;
        let (classe, membro) = match nome.split_once('.') {
            Some((c, m)) => (Some(self.classe(c)), m),
            None => (None, nome),
        };
        let f = (0..self.p.functions.len())
            .map(|i| FunctionElementId(i as u32))
            .find(|f| {
                let func = self.p.function(*f);
                !self.p.library(func.library).is_sdk
                    && self.i.resolve(func.name) == membro
                    && func.class == classe
                    && matches!(func.kind, FunctionKind::Setter) == escrita
            })
            .unwrap_or_else(|| panic!("função {nome} (escrita={escrita})"));
        self.m.funcao(f)
    }
    fn nivel(&self, nome: &str) -> NivelClasse {
        self.m.classe(self.classe(nome))
    }
}

#[test]
#[ignore = "compila contra o SDK; rodar com --ignored"]
fn casos_do_mundo_fechado() {
    let arquivo = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programas/casos.dart");
    let (r, _) = dartforge_emit_js::compilar_com(&arquivo, None, None, &Default::default(), |a| {
        let e = Entrada { program: a.program, interner: a.interner, table: a.table, outline: a.outline, bodies: a.bodies };
        let mut raizes = Raizes::default();
        let lib = a.program.entry.expect("entrada");
        let main = a.interner.lookup("main").unwrap();
        let Some(Element::Function(f)) = a.program.library(lib).declared.get(&main).and_then(|b| b.getter) else { panic!("main") };
        raizes.funcoes.push(f);
        raizes.seletores.push("toJson".into());
        let m = calcular(e, &raizes);
        let c = Caso { p: a.program, i: a.interner, m: &m };
        let mut erros: Vec<String> = Vec::new();
        let mut confere = |cond: bool, o: &str| {
            if !cond {
                erros.push(o.to_string());
            }
        };
        // dynamic por nome, em toda classe instanciada
        confere(c.viva("A.f") && c.viva("B.f"), "A.f e B.f vivos (d.f() dinâmico)");
        confere(!c.viva("A.g") && !c.viva("A.morto"), "A.g e A.morto mortos");
        confere(c.nivel("NaoInstanciada") == NivelClasse::Morta && !c.viva("NaoInstanciada.f"), "classe não instanciada morta mesmo com seletor vivo");
        confere(c.nivel("SoTipo") == NivelClasse::Tipo, "classe só em `is` é Tipo");
        confere(c.nivel("I") == NivelClasse::Tipo, "interface implementada é Tipo");
        // protocolo do SDK e de Object
        confere(c.viva("Comp.compareTo"), "compareTo vivo por Comparable (regra i)");
        confere(c.viva("Comp.toString"), "toString vivo (regra ii)");
        confere(!c.viva("Comp.naoUsado"), "Comp.naoUsado morto");
        confere(c.viva("Nsm.noSuchMethod"), "noSuchMethod vivo");
        // super, mixin
        confere(c.viva("Base.s") && c.nivel("Base") == NivelClasse::Instanciada, "Base.s vivo pela cadeia");
        confere(c.viva("M.doMixin") && !c.viva("M.mixinMorto"), "membro de mixin por seletor");
        // estáticos e topo
        confere(c.viva("Estat.usado") && !c.viva("Estat.naoUsado"), "estático usado vivo, o outro morto");
        confere(c.m.variavel(c.variavel("contador")), "estático lido/escrito vivo");
        confere(c.viva("topoUsado") && !c.viva("topoMorto") && !c.viva("topoMorto2"), "funções de topo");
        confere(!c.m.variavel(c.variavel("preguicoso")), "variável de topo não lida morta");
        confere(c.nivel("Cor") == NivelClasse::Instanciada, "enum usado instanciado");
        // extensão
        confere(c.viva("ext:grita") && !c.viva("ext:cala"), "membros de extensão por nome");
        // construtores
        confere(c.nivel("Fab") == NivelClasse::Instanciada && c.viva("Fab._"), "fábrica redirecionada instancia o alvo");
        confere(c.m.tearoff_de_construtor(c.funcao("Tear.")), "tearoff de construtor");
        confere(c.nivel("ConstUsada") == NivelClasse::Instanciada && c.nivel("ConstMorta") == NivelClasse::Morta, "const usada e não usada");
        confere(c.nivel("Json") == NivelClasse::Morta, "classe não citada morta");
        // espécie de seletor: leitura e escrita são independentes
        confere(c.viva_especie("SoLeitura.v", false) && !c.viva_especie("SoLeitura.v", true), "getter lido vive, setter nunca escrito morre");
        confere(c.viva_especie("SoEscrita.w", true) && !c.viva_especie("SoEscrita.w", false), "setter escrito vive, getter nunca lido morre");
        // restrição pelo tipo do receptor: `toca` em `Sopro` não mantém `toca` de `Flauta`
        confere(c.viva("Sopro.toca") && c.viva("Gaita.toca"), "toca vivo no cone do receptor (base e subclasse)");
        confere(!c.viva("Flauta.toca") && !c.viva("Flauta.assobia"), "toca/assobia mortos na classe disjunta instanciada");
        confere(c.nivel("Flauta") == NivelClasse::Instanciada, "Flauta instanciada mesmo com os membros podados");
        // conferência a seco e determinismo
        let inc = conferir(e, &raizes, &m);
        confere(inc.is_empty(), &format!("conferência: {inc:?}"));
        let mut perm = raizes.clone();
        perm.seletores.insert(0, "toJson".into());
        let m2 = calcular(e, &perm);
        let igual = (0..a.program.functions.len()).all(|i| m.funcao(FunctionElementId(i as u32)) == m2.funcao(FunctionElementId(i as u32)))
            && (0..a.program.classes.len()).all(|i| m.classe(ClassId(i as u32)) == m2.classe(ClassId(i as u32)));
        confere(igual, "determinismo");
        Ok(erros)
    })
    .expect("compilação");
    assert!(r.is_empty(), "falhas:\n{}", r.join("\n"));
}
