//! Tradução direta da HIR do DartForge para a IR do Cranelift.
//!
//! O ponto de partida é `dartforge_hir::Module`, a mesma estrutura que o backend
//! LLVM consome: nada de LLVM IR textual no caminho. A semântica reproduz a de
//! `dartforge-llvm` para a fatia escalar — `int` é i64 com estouro modular,
//! `bool` ocupa um byte com 0 ou 1, comparações são com sinal e a ordem de
//! avaliação é a do Dart (operando esquerdo antes do direito, argumentos na
//! ordem escrita, `&&`/`||` com curto-circuito).
//!
//! Tudo o que está fora da fatia é rejeitado com um [`Diagnostic`] que conserva
//! o span da AST, no mesmo estilo de `LLVM AOT ainda não suporta ...`.
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{AbiParam, BlockArg, InstBuilder, TrapCode, UserFuncName, types};
use cranelift_codegen::ir::{Block, Function as FuncIr, Value as ValorIr};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::JITModule;
use cranelift_module::{FuncId, Linkage, Module as _};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_hir::Module;
use dartforge_syntax::{
    BinaryOp, Expr, ExprKind, Function, ParameterKind, Statement, StatementKind, Type, UnaryOp,
};
use std::collections::HashMap;

/// Tipo da ABI interna do JIT; a fatia cobre apenas escalares e `void`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tipo {
    Int,
    Bool,
    Void,
}
impl Tipo {
    /// Tipo Cranelift correspondente; `void` não tem representação de valor.
    fn ir(self) -> types::Type {
        match self {
            Self::Int => types::I64,
            // Booleano ocupa um byte porque `icmp` do Cranelift produz i8 e a
            // ABI de `dartforge_print_bool` recebe exatamente esse byte.
            Self::Bool | Self::Void => types::I8,
        }
    }
}

/// Valor traduzido: o `ValorIr` só existe quando o tipo não é `void`.
#[derive(Clone, Copy)]
struct Valor {
    tipo: Tipo,
    ir: Option<ValorIr>,
}
impl Valor {
    /// Recupera o valor Cranelift de um operando que já se sabe não ser `void`.
    fn exigir(self, span: Span) -> Result<ValorIr, Diagnostic> {
        self.ir
            .ok_or_else(|| Diagnostic::new("valor void usado como operando na HIR Cranelift", span))
    }
}

/// Assinatura interna de uma função do programa, com símbolo numérico seguro.
struct Assinatura {
    id: FuncId,
    resultado: Tipo,
    parametros: Vec<Tipo>,
}

/// Laço aberto: destino de `break` (criado sob demanda) e destino de `continue`.
struct Laco {
    saida: Option<Block>,
    passo: Block,
}

/// Resultado da fase de tradução, antes de qualquer geração de código de máquina.
pub(crate) struct Traducao {
    /// Corpos CLIF na ordem em que devem ser definidos no módulo.
    pub(crate) corpos: Vec<(FuncId, FuncIr)>,
    /// Identificador de `dartforge_entry`, o corpo de `main`.
    pub(crate) entrada: FuncId,
    /// Instruções CLIF emitidas: contador de trabalho independente da máquina.
    pub(crate) instrucoes: usize,
}

/// Diagnóstico de limite do backend, distinto de um erro de análise do frontend.
///
/// O prefixo difere do backend LLVM de propósito: o mesmo programa pode ser
/// aceito por um e recusado pelo outro, e a mensagem precisa dizer qual recusou.
pub(crate) fn erro(span: Span, recurso: &str) -> Diagnostic {
    Diagnostic::new(format!("Cranelift JIT ainda não suporta {recurso}"), span)
}

/// Converte uma anotação de tipo do subconjunto para a ABI escalar do JIT.
fn tipo(valor: Type, span: Span) -> Result<Tipo, Diagnostic> {
    match valor {
        Type::Int => Ok(Tipo::Int),
        Type::Bool => Ok(Tipo::Bool),
        Type::Void => Ok(Tipo::Void),
        Type::Double | Type::Num | Type::NullableDouble | Type::NullableNum => {
            Err(erro(span, "double e num"))
        }
        Type::String | Type::NullableString => Err(erro(span, "strings")),
        Type::Null | Type::NullableInt | Type::NullableBool => Err(erro(span, "tipos anuláveis")),
        Type::Class(_) | Type::NullableClass(_) => Err(erro(span, "classes e enums")),
        Type::Object | Type::NullableObject => Err(erro(span, "Object")),
        Type::Parameter(_) | Type::NullableParameter(_) | Type::Applied(_) | Type::Inferred => {
            Err(erro(span, "tipos genéricos, coleções, funções e records"))
        }
        Type::Duration | Type::Timer => Err(erro(span, "Duration e Timer")),
    }
}

/// Recusa `void` onde a fatia exige um valor observável.
fn tipo_de_valor(valor: Type, span: Span) -> Result<Tipo, Diagnostic> {
    match tipo(valor, span)? {
        Tipo::Void => Err(erro(span, "valor void")),
        outro => Ok(outro),
    }
}

/// Traduz o módulo inteiro para CLIF e declara todos os símbolos no `JITModule`.
///
/// Declarar antes de traduzir é o que permite recursão e chamadas mútuas: o
/// corpo de uma função já enxerga o `FuncId` de todas as outras.
///
/// # Erros
/// Qualquer construção fora da fatia escalar produz diagnóstico com o span
/// original, inclusive em trechos que nunca seriam executados.
pub(crate) fn traduzir(modulo: &Module<'_>, jit: &mut JITModule) -> Result<Traducao, Diagnostic> {
    rejeitar_declaracoes_fora_da_fatia(modulo)?;

    let mut assinaturas: HashMap<String, Assinatura> = HashMap::new();
    let entrada = {
        let mut sig = jit.make_signature();
        sig.returns.clear();
        jit.declare_function("dartforge_entry", Linkage::Export, &sig)
            .map_err(|e| Diagnostic::new(e.to_string(), Span { start: 0, end: 0 }))?
    };
    assinaturas.insert(
        "main".to_owned(),
        Assinatura {
            id: entrada,
            resultado: Tipo::Void,
            parametros: vec![],
        },
    );
    for (indice, funcao) in modulo.functions.iter().enumerate() {
        let assinatura = declarar(jit, funcao, indice)?;
        if assinaturas
            .insert(funcao.name.to_owned(), assinatura)
            .is_some()
        {
            return Err(Diagnostic::new(
                "função duplicada na HIR Cranelift",
                funcao.span,
            ));
        }
    }

    let imprimir_i64 = declarar_runtime(jit, "dartforge_print_i64", types::I64)?;
    let imprimir_bool = declarar_runtime(jit, "dartforge_print_bool", types::I8)?;

    let mut contexto = FunctionBuilderContext::new();
    let mut corpos = Vec::with_capacity(modulo.functions.len() + 1);
    let mut instrucoes = 0;
    for (indice, funcao) in modulo.functions.iter().enumerate() {
        let assinatura = &assinaturas[funcao.name];
        let mut sig = jit.make_signature();
        for parametro in &assinatura.parametros {
            sig.params.push(AbiParam::new(parametro.ir()));
        }
        if assinatura.resultado != Tipo::Void {
            sig.returns.push(AbiParam::new(assinatura.resultado.ir()));
        } else {
            sig.returns.clear();
        }
        let mut corpo = FuncIr::with_name_signature(
            UserFuncName::user(0, u32::try_from(indice + 1).unwrap_or(u32::MAX)),
            sig,
        );
        let nomes: Vec<&str> = funcao.parameters.iter().map(|p| p.name).collect();
        instrucoes += emitir_corpo(
            &mut corpo,
            &mut contexto,
            jit,
            &assinaturas,
            imprimir_i64,
            imprimir_bool,
            assinatura.resultado,
            &nomes,
            &assinatura.parametros,
            &funcao.body,
        )?;
        corpos.push((assinaturas[funcao.name].id, corpo));
    }
    let mut sig = jit.make_signature();
    sig.returns.clear();
    let mut corpo = FuncIr::with_name_signature(UserFuncName::user(0, 0), sig);
    instrucoes += emitir_corpo(
        &mut corpo,
        &mut contexto,
        jit,
        &assinaturas,
        imprimir_i64,
        imprimir_bool,
        Tipo::Void,
        &[],
        &[],
        &modulo.statements,
    )?;
    corpos.push((entrada, corpo));
    Ok(Traducao {
        corpos,
        entrada,
        instrucoes,
    })
}

/// Recusa classes, extensions, async, genéricos e assinaturas fora da fatia.
fn rejeitar_declaracoes_fora_da_fatia(modulo: &Module<'_>) -> Result<(), Diagnostic> {
    if modulo.main_is_async || modulo.functions.iter().any(|f| f.is_async) {
        let span = modulo
            .functions
            .iter()
            .find(|f| f.is_async)
            .map_or(Span { start: 0, end: 0 }, |f| f.span);
        return Err(erro(span, "async/await"));
    }
    if let Some(classe) = modulo.classes.first() {
        // Variáveis de topo viram campos estáticos da classe sintética da
        // biblioteca; o JIT não emite armazenamento global algum.
        if classe.is_library_globals
            && let Some(campo) = classe.static_fields.first()
        {
            return Err(erro(campo.span, "variáveis de topo"));
        }
        // A biblioteca de topo vira uma classe sintética com os globais; sem
        // campos nem métodos ela não representa nenhuma declaração do usuário.
        let sintetica = classe.is_library_globals
            && classe.fields.is_empty()
            && classe.methods.is_empty()
            && classe.static_fields.is_empty()
            && classe.static_methods.is_empty()
            && classe.enum_values.is_empty();
        if !sintetica {
            return Err(erro(classe.span, "classes, enums e membros estáticos"));
        }
        if let Some(outra) = modulo.classes.get(1) {
            return Err(erro(outra.span, "classes, enums e membros estáticos"));
        }
    }
    if let Some(extension) = modulo.extensions.first() {
        return Err(erro(extension.span, "extensions"));
    }
    for funcao in &modulo.functions {
        if let Some(vinculo) = &funcao.native_binding {
            return Err(erro(vinculo.span, "@Native e ligação estática a símbolos C"));
        }
        if !funcao.type_parameters.is_empty() {
            return Err(erro(funcao.span, "funções genéricas"));
        }
        if funcao.is_getter {
            return Err(erro(funcao.span, "getters de topo"));
        }
    }
    Ok(())
}

/// Declara a assinatura de uma função do usuário sem interpolar seu nome no símbolo.
fn declarar(
    jit: &mut JITModule,
    funcao: &Function<'_>,
    indice: usize,
) -> Result<Assinatura, Diagnostic> {
    if let Some(parametro) = funcao
        .parameters
        .iter()
        .find(|p| p.kind != ParameterKind::RequiredPositional)
    {
        return Err(erro(parametro.span, "parâmetros opcionais ou nomeados"));
    }
    let resultado = tipo(funcao.return_type, funcao.span)?;
    let parametros = funcao
        .parameters
        .iter()
        .map(|p| tipo_de_valor(p.ty, p.span))
        .collect::<Result<Vec<_>, _>>()?;
    let mut sig = jit.make_signature();
    for parametro in &parametros {
        sig.params.push(AbiParam::new(parametro.ir()));
    }
    if resultado != Tipo::Void {
        sig.returns.push(AbiParam::new(resultado.ir()));
    } else {
        sig.returns.clear();
    }
    let id = jit
        .declare_function(&format!("df_fn_{indice}"), Linkage::Local, &sig)
        .map_err(|e| Diagnostic::new(e.to_string(), funcao.span))?;
    Ok(Assinatura {
        id,
        resultado,
        parametros,
    })
}

/// Declara um símbolo do runtime registrado por endereço no `JITBuilder`.
fn declarar_runtime(
    jit: &mut JITModule,
    nome: &str,
    parametro: types::Type,
) -> Result<FuncId, Diagnostic> {
    let mut sig = jit.make_signature();
    sig.params.push(AbiParam::new(parametro));
    sig.returns.clear();
    jit.declare_function(nome, Linkage::Import, &sig)
        .map_err(|e| Diagnostic::new(e.to_string(), Span { start: 0, end: 0 }))
}

/// Constrói o corpo CLIF de uma função e devolve quantas instruções emitiu.
#[allow(clippy::too_many_arguments)]
fn emitir_corpo(
    corpo: &mut FuncIr,
    contexto: &mut FunctionBuilderContext,
    jit: &mut JITModule,
    assinaturas: &HashMap<String, Assinatura>,
    imprimir_i64: FuncId,
    imprimir_bool: FuncId,
    resultado: Tipo,
    nomes: &[&str],
    parametros: &[Tipo],
    corpo_fonte: &[Statement<'_>],
) -> Result<usize, Diagnostic> {
    let config = jit.target_config();
    let mut construtor = FunctionBuilder::new(corpo, contexto);
    let entrada = construtor.create_block();
    construtor.append_block_params_for_function_params(entrada);
    construtor.switch_to_block(entrada);
    construtor.seal_block(entrada);
    let mut emissor = Emissor {
        construtor,
        jit,
        assinaturas,
        imprimir_i64,
        imprimir_bool,
        resultado,
        escopos: vec![HashMap::new()],
        lacos: vec![],
        terminado: false,
    };
    for (indice, (nome, tipo_parametro)) in nomes.iter().zip(parametros).enumerate() {
        let valor = emissor.construtor.block_params(entrada)[indice];
        let variavel = emissor.declarar_local(nome, *tipo_parametro);
        emissor.construtor.def_var(variavel, valor);
    }
    emissor.bloco(corpo_fonte)?;
    emissor.encerrar();
    let Emissor { mut construtor, .. } = emissor;
    construtor.seal_all_blocks();
    construtor.finalize(config);
    Ok(contar_instrucoes(corpo))
}

/// Conta instruções CLIF do corpo já construído, sem depender do relógio.
fn contar_instrucoes(corpo: &FuncIr) -> usize {
    corpo
        .layout
        .blocks()
        .map(|bloco| corpo.layout.block_insts(bloco).count())
        .sum()
}

/// Estado de tradução de uma única função.
struct Emissor<'a, 'b> {
    construtor: FunctionBuilder<'b>,
    jit: &'a mut JITModule,
    assinaturas: &'a HashMap<String, Assinatura>,
    imprimir_i64: FuncId,
    imprimir_bool: FuncId,
    resultado: Tipo,
    escopos: Vec<HashMap<String, (Tipo, Variable)>>,
    lacos: Vec<Laco>,
    /// Espelha `terminated` do backend LLVM: nada é emitido depois do terminador.
    terminado: bool,
}

impl Emissor<'_, '_> {
    /// Declara um local novo no escopo corrente, preservando sombreamento.
    fn declarar_local(&mut self, nome: &str, tipo_local: Tipo) -> Variable {
        let variavel = self.construtor.declare_var(tipo_local.ir());
        self.escopos
            .last_mut()
            .expect("sempre há um escopo aberto")
            .insert(nome.to_owned(), (tipo_local, variavel));
        variavel
    }

    /// Resolve o local mais próximo, do escopo interno para o externo.
    fn procurar(&self, nome: &str, span: Span) -> Result<(Tipo, Variable), Diagnostic> {
        self.escopos
            .iter()
            .rev()
            .find_map(|escopo| escopo.get(nome))
            .copied()
            .ok_or_else(|| Diagnostic::new("local não resolvido na HIR Cranelift", span))
    }

    /// Fecha a função com um terminador, mesmo em bloco inalcançável.
    fn encerrar(&mut self) {
        if self.terminado {
            return;
        }
        if self.resultado == Tipo::Void {
            self.construtor.ins().return_(&[]);
        } else {
            // O frontend garante retorno em todo caminho; este trap existe para
            // HIR construída manualmente e nunca aparece em código bem formado.
            self.construtor.ins().trap(TrapCode::unwrap_user(1));
        }
        self.terminado = true;
    }

    /// Abre um escopo léxico e traduz as instruções até o primeiro terminador.
    fn bloco(&mut self, corpo: &[Statement<'_>]) -> Result<(), Diagnostic> {
        self.escopos.push(HashMap::new());
        let resultado = self.instrucoes(corpo);
        self.escopos.pop();
        resultado
    }

    /// Traduz instruções em sequência; código morto ainda é validado pelo caminho normal.
    fn instrucoes(&mut self, corpo: &[Statement<'_>]) -> Result<(), Diagnostic> {
        for instrucao in corpo {
            if self.terminado {
                // Espelha o backend LLVM: as formas restantes ainda precisam ser
                // recusadas, mas nenhuma instrução é emitida depois do terminador.
                validar_instrucao(instrucao)?;
                continue;
            }
            self.instrucao(instrucao)?;
        }
        Ok(())
    }

    /// Traduz uma instrução da fatia e liga os blocos de controle correspondentes.
    fn instrucao(&mut self, instrucao: &Statement<'_>) -> Result<(), Diagnostic> {
        match &instrucao.kind {
            StatementKind::Variable {
                name,
                annotation,
                initializer,
                ..
            } => {
                // A anotação é conferida antes do inicializador, como faz
                // `validate_statement` do backend LLVM, para que os dois
                // recusem o mesmo ponto do programa.
                let anotado = annotation
                    .map(|t| tipo_de_valor(t, instrucao.span))
                    .transpose()?;
                let valor = self.expressao(initializer)?;
                let armazenamento = anotado.unwrap_or(valor.tipo);
                if valor.tipo != armazenamento {
                    return Err(Diagnostic::new(
                        "tipo incompatível na HIR Cranelift",
                        instrucao.span,
                    ));
                }
                let ir = valor.exigir(instrucao.span)?;
                let variavel = self.declarar_local(name, armazenamento);
                self.construtor.def_var(variavel, ir);
            }
            StatementKind::Assign { name, value } => {
                let (tipo_local, variavel) = self.procurar(name, instrucao.span)?;
                let valor = self.expressao(value)?;
                if valor.tipo != tipo_local {
                    return Err(Diagnostic::new(
                        "tipo incompatível na HIR Cranelift",
                        instrucao.span,
                    ));
                }
                let ir = valor.exigir(instrucao.span)?;
                self.construtor.def_var(variavel, ir);
            }
            StatementKind::Print(valor) => {
                let valor = self.expressao(valor)?;
                self.imprimir(valor, instrucao.span)?;
            }
            StatementKind::Expression(valor) => {
                self.expressao(valor)?;
            }
            StatementKind::Return(valor) => {
                match valor {
                    Some(valor) => {
                        let valor = self.expressao(valor)?;
                        if valor.tipo != self.resultado {
                            return Err(Diagnostic::new(
                                "tipo incompatível na HIR Cranelift",
                                instrucao.span,
                            ));
                        }
                        if self.resultado == Tipo::Void {
                            self.construtor.ins().return_(&[]);
                        } else {
                            let ir = valor.exigir(instrucao.span)?;
                            self.construtor.ins().return_(&[ir]);
                        }
                    }
                    None => {
                        if self.resultado != Tipo::Void {
                            return Err(Diagnostic::new(
                                "retorno sem valor na HIR Cranelift",
                                instrucao.span,
                            ));
                        }
                        self.construtor.ins().return_(&[]);
                    }
                }
                self.terminado = true;
            }
            StatementKind::Block(corpo) => self.bloco(corpo)?,
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => self.condicional(condition, then_body, else_body.as_deref(), instrucao.span)?,
            StatementKind::While { condition, body } => {
                self.laco(None, Some(condition), None, body, false)?;
            }
            StatementKind::DoWhile { condition, body } => {
                self.laco(None, Some(condition), None, body, true)?;
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => self.laco(
                initializer.as_deref(),
                condition.as_ref(),
                update.as_deref(),
                body,
                false,
            )?,
            StatementKind::Break => {
                let indice = self
                    .lacos
                    .len()
                    .checked_sub(1)
                    .ok_or_else(|| Diagnostic::new("break fora de laço", instrucao.span))?;
                let saida = match self.lacos[indice].saida {
                    Some(bloco) => bloco,
                    None => {
                        let bloco = self.construtor.create_block();
                        self.lacos[indice].saida = Some(bloco);
                        bloco
                    }
                };
                self.construtor.ins().jump(saida, &[]);
                self.terminado = true;
            }
            StatementKind::Continue => {
                let passo = self
                    .lacos
                    .last()
                    .map(|laco| laco.passo)
                    .ok_or_else(|| Diagnostic::new("continue fora de laço", instrucao.span))?;
                self.construtor.ins().jump(passo, &[]);
                self.terminado = true;
            }
            _ => return Err(erro(instrucao.span, recurso_de_instrucao(instrucao))),
        }
        Ok(())
    }

    /// Emite `if`/`else` unindo os ramos num bloco criado apenas se for alcançável.
    fn condicional(
        &mut self,
        condicao: &Expr<'_>,
        entao: &[Statement<'_>],
        senao: Option<&[Statement<'_>]>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let valor = self.expressao(condicao)?;
        if valor.tipo != Tipo::Bool {
            return Err(Diagnostic::new("condição não booleana na HIR Cranelift", span));
        }
        let teste = valor.exigir(span)?;
        let bloco_entao = self.construtor.create_block();
        let bloco_senao = self.construtor.create_block();
        self.construtor
            .ins()
            .brif(teste, bloco_entao, &[], bloco_senao, &[]);
        self.construtor.seal_block(bloco_entao);
        self.construtor.seal_block(bloco_senao);

        let mut fim: Option<Block> = None;
        self.construtor.switch_to_block(bloco_entao);
        self.terminado = false;
        self.bloco(entao)?;
        if !self.terminado {
            let destino = *fim.get_or_insert_with(|| self.construtor.create_block());
            self.construtor.ins().jump(destino, &[]);
        }
        self.construtor.switch_to_block(bloco_senao);
        self.terminado = false;
        if let Some(senao) = senao {
            self.bloco(senao)?;
        }
        if !self.terminado {
            let destino = *fim.get_or_insert_with(|| self.construtor.create_block());
            self.construtor.ins().jump(destino, &[]);
        }
        match fim {
            Some(destino) => {
                self.construtor.seal_block(destino);
                self.construtor.switch_to_block(destino);
                self.terminado = false;
            }
            // Os dois ramos terminaram: nada depois do `if` é alcançável.
            None => self.terminado = true,
        }
        Ok(())
    }

    /// Liga inicialização, teste, corpo e atualização de `while`, `do`/`while` e `for`.
    ///
    /// `continue` salta para o passo de atualização, como no backend LLVM, e
    /// `break` sai pelo bloco de saída, criado sob demanda quando não há teste.
    fn laco(
        &mut self,
        inicializador: Option<&Statement<'_>>,
        condicao: Option<&Expr<'_>>,
        atualizacao: Option<&Statement<'_>>,
        corpo: &[Statement<'_>],
        corpo_primeiro: bool,
    ) -> Result<(), Diagnostic> {
        self.escopos.push(HashMap::new());
        let resultado = self.laco_interno(
            inicializador,
            condicao,
            atualizacao,
            corpo,
            corpo_primeiro,
        );
        self.escopos.pop();
        resultado
    }

    /// Corpo de [`Self::laco`] separado para que o escopo feche mesmo em erro.
    fn laco_interno(
        &mut self,
        inicializador: Option<&Statement<'_>>,
        condicao: Option<&Expr<'_>>,
        atualizacao: Option<&Statement<'_>>,
        corpo: &[Statement<'_>],
        corpo_primeiro: bool,
    ) -> Result<(), Diagnostic> {
        if let Some(inicializador) = inicializador {
            self.instrucao(inicializador)?;
        }
        let teste = self.construtor.create_block();
        let trabalho = self.construtor.create_block();
        let passo = self.construtor.create_block();
        self.construtor
            .ins()
            .jump(if corpo_primeiro { trabalho } else { teste }, &[]);

        self.construtor.switch_to_block(teste);
        self.terminado = false;
        let saida = match condicao {
            Some(condicao) => {
                let valor = self.expressao(condicao)?;
                if valor.tipo != Tipo::Bool {
                    return Err(Diagnostic::new(
                        "condição não booleana na HIR Cranelift",
                        condicao.span,
                    ));
                }
                let ir = valor.exigir(condicao.span)?;
                let saida = self.construtor.create_block();
                self.construtor.ins().brif(ir, trabalho, &[], saida, &[]);
                Some(saida)
            }
            None => {
                self.construtor.ins().jump(trabalho, &[]);
                None
            }
        };

        self.lacos.push(Laco { saida, passo });
        self.construtor.switch_to_block(trabalho);
        self.construtor.seal_block(trabalho);
        self.terminado = false;
        let resultado = self.bloco(corpo);
        if resultado.is_ok() && !self.terminado {
            self.construtor.ins().jump(passo, &[]);
        }
        let laco = self.lacos.pop().expect("laço recém-empilhado");
        resultado?;

        self.construtor.seal_block(passo);
        self.construtor.switch_to_block(passo);
        self.terminado = false;
        if let Some(atualizacao) = atualizacao {
            self.instrucao(atualizacao)?;
        }
        if !self.terminado {
            self.construtor.ins().jump(teste, &[]);
        }
        self.construtor.seal_block(teste);

        match laco.saida {
            Some(saida) => {
                self.construtor.seal_block(saida);
                self.construtor.switch_to_block(saida);
                self.terminado = false;
            }
            // Laço sem teste e sem `break`: o que vem depois é inalcançável.
            None => self.terminado = true,
        }
        Ok(())
    }

    /// Chama o símbolo de impressão do runtime correspondente ao tipo do valor.
    fn imprimir(&mut self, valor: Valor, span: Span) -> Result<(), Diagnostic> {
        let alvo = match valor.tipo {
            Tipo::Int => self.imprimir_i64,
            Tipo::Bool => self.imprimir_bool,
            Tipo::Void => return Err(erro(span, "impressão de void")),
        };
        let ir = valor.exigir(span)?;
        let referencia = self
            .jit
            .declare_func_in_func(alvo, self.construtor.func);
        self.construtor.ins().call(referencia, &[ir]);
        Ok(())
    }

    /// Traduz uma expressão da fatia preservando a ordem de avaliação do Dart.
    fn expressao(&mut self, expressao: &Expr<'_>) -> Result<Valor, Diagnostic> {
        match &expressao.kind {
            ExprKind::Int(valor) => {
                let ir = self.construtor.ins().iconst(types::I64, i64::from(*valor));
                Ok(Valor {
                    tipo: Tipo::Int,
                    ir: Some(ir),
                })
            }
            ExprKind::Bool(valor) => {
                let ir = self.construtor.ins().iconst(types::I8, i64::from(*valor));
                Ok(Valor {
                    tipo: Tipo::Bool,
                    ir: Some(ir),
                })
            }
            ExprKind::Identifier(nome) => {
                let (tipo_local, variavel) = self.procurar(nome, expressao.span)?;
                let ir = self.construtor.use_var(variavel);
                Ok(Valor {
                    tipo: tipo_local,
                    ir: Some(ir),
                })
            }
            ExprKind::Call { name, arguments } => self.chamada(name, arguments, expressao.span),
            ExprKind::Unary { op, operand } => {
                let valor = self.expressao(operand)?;
                match op {
                    UnaryOp::Negate => {
                        if valor.tipo != Tipo::Int {
                            return Err(Diagnostic::new(
                                "tipo incompatível na HIR Cranelift",
                                expressao.span,
                            ));
                        }
                        let ir = valor.exigir(expressao.span)?;
                        // `ineg` é `0 - x` em complemento de dois: o mesmo
                        // estouro modular que `sub i64 0, x` no backend LLVM.
                        let ir = self.construtor.ins().ineg(ir);
                        Ok(Valor {
                            tipo: Tipo::Int,
                            ir: Some(ir),
                        })
                    }
                    UnaryOp::Not => {
                        if valor.tipo != Tipo::Bool {
                            return Err(Diagnostic::new(
                                "tipo incompatível na HIR Cranelift",
                                expressao.span,
                            ));
                        }
                        let ir = valor.exigir(expressao.span)?;
                        // O operando só assume 0 ou 1, então o xor com 1 nega.
                        let ir = self.construtor.ins().bxor_imm_u(ir, 1);
                        Ok(Valor {
                            tipo: Tipo::Bool,
                            ir: Some(ir),
                        })
                    }
                    UnaryOp::NullAssert => Err(erro(expressao.span, "o operador `!` de não nulo")),
                    UnaryOp::BitNot => Err(erro(expressao.span, "operadores bit a bit")),
                }
            }
            ExprKind::Binary { op, left, right } => self.binaria(*op, left, right, expressao.span),
            _ => Err(erro(expressao.span, recurso_de_expressao(expressao))),
        }
    }

    /// Traduz `print` e as chamadas às funções do próprio programa, inclusive recursivas.
    fn chamada(
        &mut self,
        nome: &str,
        argumentos: &[Expr<'_>],
        span: Span,
    ) -> Result<Valor, Diagnostic> {
        if nome == "print" {
            if argumentos.len() != 1 {
                return Err(Diagnostic::new("print exige um argumento", span));
            }
            let valor = self.expressao(&argumentos[0])?;
            self.imprimir(valor, span)?;
            return Ok(Valor {
                tipo: Tipo::Void,
                ir: None,
            });
        }
        let assinatura = self
            .assinaturas
            .get(nome)
            .ok_or_else(|| Diagnostic::new("função não resolvida na HIR Cranelift", span))?;
        if argumentos.len() != assinatura.parametros.len() {
            return Err(Diagnostic::new("aridade incorreta na HIR Cranelift", span));
        }
        let (id, resultado) = (assinatura.id, assinatura.resultado);
        let esperados = assinatura.parametros.clone();
        let mut valores = Vec::with_capacity(argumentos.len());
        for (argumento, esperado) in argumentos.iter().zip(&esperados) {
            let valor = self.expressao(argumento)?;
            if valor.tipo != *esperado {
                return Err(Diagnostic::new("tipo incompatível na HIR Cranelift", span));
            }
            valores.push(valor.exigir(span)?);
        }
        let referencia = self.jit.declare_func_in_func(id, self.construtor.func);
        let chamada = self.construtor.ins().call(referencia, &valores);
        if resultado == Tipo::Void {
            return Ok(Valor {
                tipo: Tipo::Void,
                ir: None,
            });
        }
        let ir = self.construtor.inst_results(chamada)[0];
        Ok(Valor {
            tipo: resultado,
            ir: Some(ir),
        })
    }

    /// Traduz operadores binários, com curto-circuito em `&&` e `||`.
    fn binaria(
        &mut self,
        op: BinaryOp,
        esquerda: &Expr<'_>,
        direita: &Expr<'_>,
        span: Span,
    ) -> Result<Valor, Diagnostic> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            return self.curto_circuito(op, esquerda, direita, span);
        }
        // Operadores fora da fatia são recusados antes de traduzir os operandos,
        // como faz `validate_expression` do backend LLVM.
        recusar_operador_binario(op, span)?;
        let valor_esquerdo = self.expressao(esquerda)?;
        let valor_direito = self.expressao(direita)?;
        if valor_esquerdo.tipo != valor_direito.tipo || valor_esquerdo.tipo == Tipo::Void {
            return Err(Diagnostic::new("tipo incompatível na HIR Cranelift", span));
        }
        let e = valor_esquerdo.exigir(span)?;
        let d = valor_direito.exigir(span)?;
        let escalar = valor_esquerdo.tipo;
        let aritmetica = matches!(
            op,
            BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
        );
        if aritmetica && escalar != Tipo::Int {
            return Err(Diagnostic::new("tipo incompatível na HIR Cranelift", span));
        }
        let (ir, tipo_resultado) = match op {
            // iadd/isub/imul do Cranelift são modulares em complemento de dois,
            // exatamente como add/sub/mul sem nsw/nuw do backend LLVM.
            BinaryOp::Add => (self.construtor.ins().iadd(e, d), Tipo::Int),
            BinaryOp::Subtract => (self.construtor.ins().isub(e, d), Tipo::Int),
            BinaryOp::Multiply => (self.construtor.ins().imul(e, d), Tipo::Int),
            BinaryOp::Equal => (
                self.construtor.ins().icmp(IntCC::Equal, e, d),
                Tipo::Bool,
            ),
            BinaryOp::NotEqual => (
                self.construtor.ins().icmp(IntCC::NotEqual, e, d),
                Tipo::Bool,
            ),
            BinaryOp::Less => (
                self.construtor.ins().icmp(IntCC::SignedLessThan, e, d),
                Tipo::Bool,
            ),
            BinaryOp::LessEqual => (
                self.construtor
                    .ins()
                    .icmp(IntCC::SignedLessThanOrEqual, e, d),
                Tipo::Bool,
            ),
            BinaryOp::Greater => (
                self.construtor.ins().icmp(IntCC::SignedGreaterThan, e, d),
                Tipo::Bool,
            ),
            BinaryOp::GreaterEqual => (
                self.construtor
                    .ins()
                    .icmp(IntCC::SignedGreaterThanOrEqual, e, d),
                Tipo::Bool,
            ),
            BinaryOp::Remainder
            | BinaryOp::Divide
            | BinaryOp::TruncDivide
            | BinaryOp::IfNull
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
            | BinaryOp::And
            | BinaryOp::Or => unreachable!("recusado ou tratado antes dos operandos"),
        };
        Ok(Valor {
            tipo: tipo_resultado,
            ir: Some(ir),
        })
    }

    /// Emite `&&` e `||` avaliando o operando direito só quando necessário.
    ///
    /// O bloco de junção recebe o resultado como parâmetro, que é a forma
    /// Cranelift do `phi` emitido pelo backend LLVM para os mesmos operadores.
    fn curto_circuito(
        &mut self,
        op: BinaryOp,
        esquerda: &Expr<'_>,
        direita: &Expr<'_>,
        span: Span,
    ) -> Result<Valor, Diagnostic> {
        let valor = self.expressao(esquerda)?;
        if valor.tipo != Tipo::Bool {
            return Err(Diagnostic::new("tipo incompatível na HIR Cranelift", span));
        }
        let teste = valor.exigir(span)?;
        let bloco_direito = self.construtor.create_block();
        let fim = self.construtor.create_block();
        self.construtor.append_block_param(fim, types::I8);
        let curto = self.construtor.ins().iconst(
            types::I8,
            i64::from(op == BinaryOp::Or),
        );
        if op == BinaryOp::And {
            self.construtor
                .ins()
                .brif(teste, bloco_direito, &[], fim, &[BlockArg::Value(curto)]);
        } else {
            self.construtor
                .ins()
                .brif(teste, fim, &[BlockArg::Value(curto)], bloco_direito, &[]);
        }
        self.construtor.seal_block(bloco_direito);
        self.construtor.switch_to_block(bloco_direito);
        let valor = self.expressao(direita)?;
        if valor.tipo != Tipo::Bool {
            return Err(Diagnostic::new("tipo incompatível na HIR Cranelift", span));
        }
        let ir = valor.exigir(span)?;
        self.construtor.ins().jump(fim, &[BlockArg::Value(ir)]);
        self.construtor.seal_block(fim);
        self.construtor.switch_to_block(fim);
        Ok(Valor {
            tipo: Tipo::Bool,
            ir: Some(self.construtor.block_params(fim)[0]),
        })
    }
}

/// Recusa os operadores binários fora da fatia antes de traduzir os operandos.
///
/// O `match` é exaustivo de propósito: um operador novo no `dartforge-syntax`
/// quebra a compilação deste crate em vez de passar despercebido.
fn recusar_operador_binario(op: BinaryOp, span: Span) -> Result<(), Diagnostic> {
    match op {
        BinaryOp::Remainder => Err(erro(span, "módulo euclidiano")),
        BinaryOp::Divide | BinaryOp::TruncDivide => {
            Err(erro(span, "divisão double e truncada (`/`, `~/`)"))
        }
        BinaryOp::IfNull => Err(erro(span, "o operador `??`")),
        BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight => Err(erro(span, "operadores bit a bit e deslocamentos")),
        BinaryOp::Add
        | BinaryOp::Subtract
        | BinaryOp::Multiply
        | BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual
        | BinaryOp::And
        | BinaryOp::Or => Ok(()),
    }
}

/// Nome do recurso recusado numa instrução, para a mensagem de diagnóstico.
fn recurso_de_instrucao(instrucao: &Statement<'_>) -> &'static str {
    match &instrucao.kind {
        StatementKind::Switch { .. } => "switch",
        StatementKind::Try { .. } | StatementKind::Rethrow => "try, catch, finally e rethrow",
        StatementKind::Assert { .. } => "assert",
        StatementKind::ForIn { .. } => "for-in",
        StatementKind::Labeled { .. }
        | StatementKind::BreakLabel(_)
        | StatementKind::ContinueLabel(_) => "rótulos de laço",
        StatementKind::RecordDestructure { .. } => "desestruturação de records",
        StatementKind::IndexAssign { .. } => "atribuição por índice",
        StatementKind::FieldAssign { .. } => "atribuição de campo",
        _ => "esta instrução",
    }
}

/// Nome do recurso recusado numa expressão, para a mensagem de diagnóstico.
fn recurso_de_expressao(expressao: &Expr<'_>) -> &'static str {
    match &expressao.kind {
        ExprKind::Double(_) => "literais double",
        ExprKind::String(_) | ExprKind::OwnedString(_) => "strings",
        ExprKind::Interpolation(_) => "interpolação de strings",
        ExprKind::Null => "null",
        ExprKind::Conditional { .. } => "o operador condicional",
        ExprKind::Throw(_) => "throw",
        ExprKind::List { .. } | ExprKind::Map { .. } | ExprKind::Index { .. } => "coleções",
        ExprKind::Closure { .. } | ExprKind::Invoke { .. } => "closures",
        ExprKind::Record { .. } => "records",
        ExprKind::Switch { .. } => "switch",
        ExprKind::TypeTest { .. } | ExprKind::Cast { .. } => "testes e casts de tipos",
        ExprKind::GenericCall { .. } => "chamadas genéricas",
        ExprKind::NamedArgument { .. } => "argumentos nomeados",
        ExprKind::Await(_)
        | ExprKind::FutureValue { .. }
        | ExprKind::FutureDelayed { .. }
        | ExprKind::Duration { .. } => "operações assíncronas",
        ExprKind::Cascade { .. } | ExprKind::CascadeReceiver => "cascatas",
        ExprKind::Const(_) => "expressões const",
        ExprKind::This
        | ExprKind::Construct { .. }
        | ExprKind::NamedConstruct { .. }
        | ExprKind::Member { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::EnumValue { .. }
        | ExprKind::DotShorthand { .. } => "classes e enums",
        ExprKind::NullAwareElement(_) => "elementos null-aware de coleções",
        _ => "esta expressão",
    }
}

/// Valida instruções que ficam depois de um terminador, sem emitir nada.
///
/// O backend LLVM percorre todo o corpo em `validate_statements` antes de
/// emitir; aqui a validação de código morto acontece neste ponto, o que
/// preserva a garantia de que nenhuma forma fora da fatia passa despercebida.
fn validar_instrucao(instrucao: &Statement<'_>) -> Result<(), Diagnostic> {
    match &instrucao.kind {
        StatementKind::Variable { initializer, .. } => validar_expressao(initializer),
        StatementKind::Assign { value, .. }
        | StatementKind::Print(value)
        | StatementKind::Expression(value) => validar_expressao(value),
        StatementKind::Return(valor) => valor.as_ref().map_or(Ok(()), validar_expressao),
        StatementKind::Block(corpo) => corpo.iter().try_for_each(validar_instrucao),
        StatementKind::If {
            condition,
            then_body,
            else_body,
        } => {
            validar_expressao(condition)?;
            then_body.iter().try_for_each(validar_instrucao)?;
            else_body
                .iter()
                .flatten()
                .try_for_each(validar_instrucao)
        }
        StatementKind::While { condition, body } | StatementKind::DoWhile { condition, body } => {
            validar_expressao(condition)?;
            body.iter().try_for_each(validar_instrucao)
        }
        StatementKind::For {
            initializer,
            condition,
            update,
            body,
        } => {
            initializer
                .as_deref()
                .map_or(Ok(()), validar_instrucao)?;
            condition.as_ref().map_or(Ok(()), validar_expressao)?;
            update.as_deref().map_or(Ok(()), validar_instrucao)?;
            body.iter().try_for_each(validar_instrucao)
        }
        StatementKind::Break | StatementKind::Continue => Ok(()),
        _ => Err(erro(instrucao.span, recurso_de_instrucao(instrucao))),
    }
}

/// Valida expressões de código morto com as mesmas regras da tradução.
fn validar_expressao(expressao: &Expr<'_>) -> Result<(), Diagnostic> {
    match &expressao.kind {
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Identifier(_) => Ok(()),
        ExprKind::Call { arguments, .. } => arguments.iter().try_for_each(validar_expressao),
        ExprKind::Unary { op, operand } => {
            match op {
                UnaryOp::NullAssert => {
                    return Err(erro(expressao.span, "o operador `!` de não nulo"));
                }
                UnaryOp::BitNot => return Err(erro(expressao.span, "operadores bit a bit")),
                UnaryOp::Negate | UnaryOp::Not => {}
            }
            validar_expressao(operand)
        }
        ExprKind::Binary { op, left, right } => {
            recusar_operador_binario(*op, expressao.span)?;
            validar_expressao(left)?;
            validar_expressao(right)
        }
        _ => Err(erro(expressao.span, recurso_de_expressao(expressao))),
    }
}
