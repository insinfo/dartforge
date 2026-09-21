//! Tradução direta da HIR do DartForge para instruções x86-64, sem IR no meio.
//!
//! O ponto de partida é `dartforge_hir::Module`, a mesma estrutura que o backend
//! LLVM consome: nada de LLVM IR textual no caminho. A diferença para os outros
//! dois experimentos é que aqui **não existe representação intermediária**. Um
//! montador codifica cada instrução no ato; o que este módulo escreve já é o
//! código final. No caso do dynasm-rs a codificação é resolvida ainda antes:
//! `dynasm!` é uma macro procedural, e em tempo de execução só restam os
//! operandos variáveis.
//!
//! # O modelo de execução escolhido
//!
//! Sem alocador de registradores, a escolha é entre inventar um ou não precisar
//! de um. Este tradutor não precisa: **toda posição viva mora na pilha** e
//! `RAX` é o único acumulador.
//!
//! * cada parâmetro e cada declaração de variável recebe uma posição fixa de 8
//!   bytes relativa a `RBP`, calculada antes do prólogo por [`crate::quadro`];
//! * uma expressão deixa seu valor em `RAX`;
//! * numa operação binária o operando esquerdo é derramado numa posição
//!   temporária enquanto o direito é avaliado, e depois relido em `RAX` com o
//!   direito em `RCX`.
//!
//! O código resultante é volumoso e cheio de idas à pilha — é exatamente esse o
//! preço que `docs/ASMJIT.md` mede no eixo 2. A vantagem é que o gerador não
//! tem estado global nem análise: a tradução é uma única passagem.
//!
//! A semântica reproduz a de `dartforge-llvm` para a fatia escalar: `int` é i64
//! com estouro modular (`add`/`sub`/`imul` são modulares em complemento de
//! dois), `bool` é 0 ou 1, comparações são com sinal e a ordem de avaliação é a
//! do Dart (esquerdo antes do direito, argumentos na ordem escrita, `&&`/`||`
//! com curto-circuito).
//!
//! Tudo o que está fora da fatia é rejeitado com um [`Diagnostic`] que conserva
//! o span da AST, no mesmo estilo de `LLVM AOT ainda não suporta ...`.
#![allow(clippy::useless_conversion)]
use crate::abi::{ARGUMENTOS, MAXIMO_DE_PARAMETROS, tamanho_do_quadro};
use crate::quadro;
use crate::runtime;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_hir::Module;
use dartforge_syntax::{
    BinaryOp, Expr, ExprKind, Function, ParameterKind, Statement, StatementKind, Type, UnaryOp,
};
use dynasmrt::x64::Assembler;
use dynasmrt::{AssemblyOffset, DynamicLabel, DynasmApi, DynasmLabelApi, dynasm};
use std::collections::HashMap;

/// Emite uma instrução e contabiliza o trabalho num contador independente da máquina.
///
/// Cada uso emite exatamente uma instrução, para que o contador conte
/// instruções de verdade e não chamadas de macro.
macro_rules! emitir {
    ($emissor:ident $($resto:tt)*) => {{
        $emissor.instrucoes += 1;
        dynasm!($emissor.ops ; .arch x64 $($resto)*);
    }};
}

/// Tipo da ABI interna do JIT; a fatia cobre apenas escalares e `void`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tipo {
    Int,
    Bool,
    Void,
}

/// Valor traduzido: quando o tipo não é `void`, ele está em `RAX`.
#[derive(Clone, Copy)]
struct Valor {
    tipo: Tipo,
}

/// Assinatura interna de uma função do programa, com o rótulo do seu prólogo.
struct Assinatura {
    rotulo: DynamicLabel,
    resultado: Tipo,
    parametros: Vec<Tipo>,
}

/// Laço aberto: destino de `break` e destino de `continue`.
struct Laco {
    saida: DynamicLabel,
    passo: DynamicLabel,
    /// `break` ou a condição referenciaram a saída; se ninguém referenciou, o
    /// que vem depois do laço é inalcançável.
    saida_usada: bool,
}

/// Resultado da tradução, antes de tornar o bloco executável.
pub(crate) struct Traducao {
    /// Deslocamento do prólogo do corpo de `main` dentro do bloco emitido.
    pub(crate) entrada: AssemblyOffset,
    /// Instruções x86-64 emitidas: contador de trabalho independente da máquina.
    pub(crate) instrucoes: usize,
}

/// Diagnóstico de limite do backend, distinto de um erro de análise do frontend.
///
/// O prefixo difere dos outros backends de propósito: o mesmo programa pode ser
/// aceito por um e recusado pelo outro, e a mensagem precisa dizer qual recusou.
pub(crate) fn erro(span: Span, recurso: &str) -> Diagnostic {
    Diagnostic::new(format!("asmjit JIT ainda não suporta {recurso}"), span)
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

/// Traduz o módulo inteiro para código de máquina no montador recebido.
///
/// Os rótulos de todas as funções são criados antes de qualquer corpo, o que é
/// o que permite recursão e chamadas mútuas: um `call` para um rótulo ainda não
/// vinculado vira uma relocação que o montador resolve ao confirmar o bloco.
///
/// # Erros
/// Qualquer construção fora da fatia escalar produz diagnóstico com o span
/// original, inclusive em trechos que nunca seriam executados.
pub(crate) fn traduzir(modulo: &Module<'_>, ops: &mut Assembler) -> Result<Traducao, Diagnostic> {
    rejeitar_declaracoes_fora_da_fatia(modulo)?;

    let mut assinaturas: HashMap<String, Assinatura> = HashMap::new();
    assinaturas.insert(
        "main".to_owned(),
        Assinatura {
            rotulo: ops.new_dynamic_label(),
            resultado: Tipo::Void,
            parametros: vec![],
        },
    );
    for funcao in &modulo.functions {
        let assinatura = declarar(ops, funcao)?;
        if assinaturas
            .insert(funcao.name.to_owned(), assinatura)
            .is_some()
        {
            return Err(Diagnostic::new(
                "função duplicada na HIR asmjit",
                funcao.span,
            ));
        }
    }

    let mut instrucoes = 0;
    for funcao in &modulo.functions {
        let assinatura = &assinaturas[funcao.name];
        let rotulo = assinatura.rotulo;
        let resultado = assinatura.resultado;
        let parametros = assinatura.parametros.clone();
        let nomes: Vec<&str> = funcao.parameters.iter().map(|p| p.name).collect();
        instrucoes += emitir_corpo(
            ops,
            &assinaturas,
            rotulo,
            resultado,
            &nomes,
            &parametros,
            &funcao.body,
        )?;
    }
    let entrada = ops.offset();
    let rotulo_entrada = assinaturas["main"].rotulo;
    instrucoes += emitir_corpo(
        ops,
        &assinaturas,
        rotulo_entrada,
        Tipo::Void,
        &[],
        &[],
        &modulo.statements,
    )?;

    Ok(Traducao {
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
            return Err(erro(
                vinculo.span,
                "@Native e ligação estática a símbolos C",
            ));
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

/// Cria o rótulo e valida a assinatura de uma função do usuário.
fn declarar(ops: &mut Assembler, funcao: &Function<'_>) -> Result<Assinatura, Diagnostic> {
    if let Some(parametro) = funcao
        .parameters
        .iter()
        .find(|p| p.kind != ParameterKind::RequiredPositional)
    {
        return Err(erro(parametro.span, "parâmetros opcionais ou nomeados"));
    }
    if let Some(parametro) = funcao.parameters.get(MAXIMO_DE_PARAMETROS) {
        return Err(erro(parametro.span, "mais de quatro parâmetros por função"));
    }
    let resultado = tipo(funcao.return_type, funcao.span)?;
    let parametros = funcao
        .parameters
        .iter()
        .map(|p| tipo_de_valor(p.ty, p.span))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Assinatura {
        rotulo: ops.new_dynamic_label(),
        resultado,
        parametros,
    })
}

/// Emite prólogo, corpo e epílogo de uma função e devolve quantas instruções emitiu.
#[allow(clippy::too_many_arguments)]
fn emitir_corpo(
    ops: &mut Assembler,
    assinaturas: &HashMap<String, Assinatura>,
    rotulo: DynamicLabel,
    resultado: Tipo,
    nomes: &[&str],
    parametros: &[Tipo],
    corpo: &[Statement<'_>],
) -> Result<usize, Diagnostic> {
    let locais = quadro::contar_locais(corpo);
    let temporarios = quadro::profundidade(corpo);
    let base_temporarios = parametros.len() + locais;
    let bytes = tamanho_do_quadro(base_temporarios + temporarios);

    let mut emissor = Emissor {
        ops,
        assinaturas,
        escopos: vec![HashMap::new()],
        proximo_local: parametros.len(),
        base_temporarios,
        profundidade_atual: 0,
        resultado,
        lacos: vec![],
        terminado: false,
        instrucoes: 0,
    };

    emissor.vincular(rotulo);
    emitir!(emissor ; push rbp);
    emitir!(emissor ; mov rbp, rsp);
    emitir!(emissor ; sub rsp, DWORD bytes);
    for (indice, (nome, tipo_parametro)) in nomes.iter().zip(parametros).enumerate() {
        let posicao = deslocamento(indice);
        let registrador = ARGUMENTOS[indice];
        emitir!(emissor ; mov QWORD [rbp + posicao], Rq(registrador));
        emissor
            .escopos
            .last_mut()
            .expect("sempre há um escopo aberto")
            .insert((*nome).to_owned(), (*tipo_parametro, indice));
    }
    emissor.bloco(corpo)?;
    emissor.encerrar();
    Ok(emissor.instrucoes)
}

/// Deslocamento, relativo a `RBP`, da posição de 8 bytes de índice `indice`.
///
/// As posições crescem para baixo a partir de `RBP`, que aponta para o `RBP`
/// salvo pelo prólogo: a posição 0 fica em `[rbp-8]`.
///
/// # Panics
/// Entra em pânico se o índice não couber em `i32`, o que exigiria uma função
/// com centenas de milhões de posições.
fn deslocamento(indice: usize) -> i32 {
    let indice = i32::try_from(indice).expect("índice de posição maior que i32");
    -8 * (indice + 1)
}

/// Estado de tradução de uma única função.
struct Emissor<'a> {
    ops: &'a mut Assembler,
    assinaturas: &'a HashMap<String, Assinatura>,
    escopos: Vec<HashMap<String, (Tipo, usize)>>,
    /// Próxima posição livre para uma declaração de variável.
    proximo_local: usize,
    /// Primeira posição da pilha de avaliação de expressões.
    base_temporarios: usize,
    /// Quantas posições temporárias estão ocupadas neste ponto da expressão.
    profundidade_atual: usize,
    resultado: Tipo,
    lacos: Vec<Laco>,
    /// Espelha `terminated` do backend LLVM: nada é emitido depois do terminador.
    terminado: bool,
    instrucoes: usize,
}

impl Emissor<'_> {
    /// Cria um rótulo dinâmico ainda não posicionado.
    fn novo_rotulo(&mut self) -> DynamicLabel {
        self.ops.new_dynamic_label()
    }

    /// Fixa o rótulo no ponto corrente; não emite instrução nenhuma.
    fn vincular(&mut self, rotulo: DynamicLabel) {
        dynasm!(self.ops ; .arch x64 ; =>rotulo);
    }

    /// Declara um local novo no escopo corrente, preservando sombreamento.
    fn declarar_local(&mut self, nome: &str, tipo_local: Tipo) -> usize {
        let indice = self.proximo_local;
        self.proximo_local += 1;
        self.escopos
            .last_mut()
            .expect("sempre há um escopo aberto")
            .insert(nome.to_owned(), (tipo_local, indice));
        indice
    }

    /// Resolve o local mais próximo, do escopo interno para o externo.
    fn procurar(&self, nome: &str, span: Span) -> Result<(Tipo, usize), Diagnostic> {
        self.escopos
            .iter()
            .rev()
            .find_map(|escopo| escopo.get(nome))
            .copied()
            .ok_or_else(|| Diagnostic::new("local não resolvido na HIR asmjit", span))
    }

    /// Lê a posição `indice` do quadro para `RAX`.
    fn carregar(&mut self, indice: usize) {
        let posicao = deslocamento(indice);
        emitir!(self ; mov rax, QWORD [rbp + posicao]);
    }

    /// Escreve `RAX` na posição `indice` do quadro.
    fn guardar(&mut self, indice: usize) {
        let posicao = deslocamento(indice);
        emitir!(self ; mov QWORD [rbp + posicao], rax);
    }

    /// Emite `mov rsp, rbp; pop rbp; ret`, o epílogo único desta fatia.
    fn epilogo(&mut self) {
        emitir!(self ; mov rsp, rbp);
        emitir!(self ; pop rbp);
        emitir!(self ; ret);
    }

    /// Fecha a função com um terminador, mesmo em bloco inalcançável.
    ///
    /// Uma função com retorno escalar cujo fluxo escapa sem `return` não existe
    /// em programa bem formado — o frontend exige retorno em todo caminho — mas
    /// o epílogo é emitido mesmo assim para que a execução nunca caia no
    /// prólogo da função seguinte do bloco emitido.
    fn encerrar(&mut self) {
        if self.terminado {
            return;
        }
        emitir!(self ; xor rax, rax);
        self.epilogo();
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

    /// Traduz uma instrução da fatia e liga os rótulos de controle correspondentes.
    fn instrucao(&mut self, instrucao: &Statement<'_>) -> Result<(), Diagnostic> {
        match &instrucao.kind {
            // `late` não tem célula de inicialização aqui: recusar é o que
            // impede a declaração de virar `null` e a leitura antes da escrita
            // de devolver null em vez de lançar.
            StatementKind::Variable { is_late: true, .. } => {
                return Err(erro(instrucao.span, "late"));
            }
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
                if valor.tipo != armazenamento || armazenamento == Tipo::Void {
                    return Err(Diagnostic::new(
                        "tipo incompatível na HIR asmjit",
                        instrucao.span,
                    ));
                }
                let indice = self.declarar_local(name, armazenamento);
                self.guardar(indice);
            }
            StatementKind::Assign { name, value } => {
                let (tipo_local, indice) = self.procurar(name, instrucao.span)?;
                let valor = self.expressao(value)?;
                if valor.tipo != tipo_local {
                    return Err(Diagnostic::new(
                        "tipo incompatível na HIR asmjit",
                        instrucao.span,
                    ));
                }
                self.guardar(indice);
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
                                "tipo incompatível na HIR asmjit",
                                instrucao.span,
                            ));
                        }
                    }
                    None => {
                        if self.resultado != Tipo::Void {
                            return Err(Diagnostic::new(
                                "retorno sem valor na HIR asmjit",
                                instrucao.span,
                            ));
                        }
                    }
                }
                self.epilogo();
                self.terminado = true;
            }
            StatementKind::Block(corpo) => self.bloco(corpo)?,
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => self.condicional(condition, then_body, else_body.as_deref())?,
            StatementKind::While { condition, body } => self.enquanto(condition, body)?,
            StatementKind::DoWhile { condition, body } => self.faca_enquanto(condition, body)?,
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => self.para(
                initializer.as_deref(),
                condition.as_ref(),
                update.as_deref(),
                body,
            )?,
            StatementKind::Break => {
                let indice = self
                    .lacos
                    .len()
                    .checked_sub(1)
                    .ok_or_else(|| Diagnostic::new("break fora de laço", instrucao.span))?;
                self.lacos[indice].saida_usada = true;
                let saida = self.lacos[indice].saida;
                emitir!(self ; jmp =>saida);
                self.terminado = true;
            }
            StatementKind::Continue => {
                let passo = self
                    .lacos
                    .last()
                    .map(|laco| laco.passo)
                    .ok_or_else(|| Diagnostic::new("continue fora de laço", instrucao.span))?;
                emitir!(self ; jmp =>passo);
                self.terminado = true;
            }
            _ => return Err(erro(instrucao.span, recurso_de_instrucao(instrucao))),
        }
        Ok(())
    }

    /// Avalia a condição e salta para `destino` quando ela é falsa.
    fn saltar_se_falso(
        &mut self,
        condicao: &Expr<'_>,
        destino: DynamicLabel,
    ) -> Result<(), Diagnostic> {
        let valor = self.expressao(condicao)?;
        if valor.tipo != Tipo::Bool {
            return Err(Diagnostic::new(
                "condição não booleana na HIR asmjit",
                condicao.span,
            ));
        }
        emitir!(self ; test rax, rax);
        emitir!(self ; jz =>destino);
        Ok(())
    }

    /// Emite `if`/`else` unindo os ramos num rótulo criado apenas se for alcançável.
    fn condicional(
        &mut self,
        condicao: &Expr<'_>,
        entao: &[Statement<'_>],
        senao: Option<&[Statement<'_>]>,
    ) -> Result<(), Diagnostic> {
        let bloco_senao = self.novo_rotulo();
        self.saltar_se_falso(condicao, bloco_senao)?;

        let mut fim: Option<DynamicLabel> = None;
        self.terminado = false;
        self.bloco(entao)?;
        if !self.terminado {
            let destino = match fim {
                Some(destino) => destino,
                None => *fim.insert(self.novo_rotulo()),
            };
            emitir!(self ; jmp =>destino);
        }
        self.vincular(bloco_senao);
        self.terminado = false;
        if let Some(senao) = senao {
            self.bloco(senao)?;
        }
        if !self.terminado {
            let destino = match fim {
                Some(destino) => destino,
                None => *fim.insert(self.novo_rotulo()),
            };
            emitir!(self ; jmp =>destino);
        }
        match fim {
            Some(destino) => {
                self.vincular(destino);
                self.terminado = false;
            }
            // Os dois ramos terminaram: nada depois do `if` é alcançável.
            None => self.terminado = true,
        }
        Ok(())
    }

    /// `while`: o teste é o destino de `continue`, como no backend LLVM.
    fn enquanto(&mut self, condicao: &Expr<'_>, corpo: &[Statement<'_>]) -> Result<(), Diagnostic> {
        let teste = self.novo_rotulo();
        let saida = self.novo_rotulo();
        self.vincular(teste);
        self.saltar_se_falso(condicao, saida)?;
        self.lacos.push(Laco {
            saida,
            passo: teste,
            saida_usada: true,
        });
        self.terminado = false;
        let resultado = self.bloco(corpo);
        if resultado.is_ok() && !self.terminado {
            emitir!(self ; jmp =>teste);
        }
        self.lacos.pop().expect("laço recém-empilhado");
        resultado?;
        self.vincular(saida);
        self.terminado = false;
        Ok(())
    }

    /// `do`/`while`: o corpo roda antes do primeiro teste.
    fn faca_enquanto(
        &mut self,
        condicao: &Expr<'_>,
        corpo: &[Statement<'_>],
    ) -> Result<(), Diagnostic> {
        let inicio = self.novo_rotulo();
        let teste = self.novo_rotulo();
        let saida = self.novo_rotulo();
        self.vincular(inicio);
        self.lacos.push(Laco {
            saida,
            passo: teste,
            saida_usada: true,
        });
        self.terminado = false;
        let resultado = self.bloco(corpo);
        if resultado.is_ok() && !self.terminado {
            emitir!(self ; jmp =>teste);
        }
        self.lacos.pop().expect("laço recém-empilhado");
        resultado?;
        self.vincular(teste);
        self.saltar_se_falso(condicao, saida)?;
        emitir!(self ; jmp =>inicio);
        self.vincular(saida);
        self.terminado = false;
        Ok(())
    }

    /// `for`: `continue` salta para a atualização, como no backend LLVM.
    fn para(
        &mut self,
        inicializador: Option<&Statement<'_>>,
        condicao: Option<&Expr<'_>>,
        atualizacao: Option<&Statement<'_>>,
        corpo: &[Statement<'_>],
    ) -> Result<(), Diagnostic> {
        self.escopos.push(HashMap::new());
        let resultado = self.para_interno(inicializador, condicao, atualizacao, corpo);
        self.escopos.pop();
        resultado
    }

    /// Corpo de [`Self::para`] separado para que o escopo feche mesmo em erro.
    fn para_interno(
        &mut self,
        inicializador: Option<&Statement<'_>>,
        condicao: Option<&Expr<'_>>,
        atualizacao: Option<&Statement<'_>>,
        corpo: &[Statement<'_>],
    ) -> Result<(), Diagnostic> {
        if let Some(inicializador) = inicializador {
            self.instrucao(inicializador)?;
        }
        let teste = self.novo_rotulo();
        let passo = self.novo_rotulo();
        let saida = self.novo_rotulo();
        self.vincular(teste);
        let com_teste = condicao.is_some();
        if let Some(condicao) = condicao {
            self.saltar_se_falso(condicao, saida)?;
        }
        self.lacos.push(Laco {
            saida,
            passo,
            saida_usada: com_teste,
        });
        self.terminado = false;
        let resultado = self.bloco(corpo);
        if resultado.is_ok() && !self.terminado {
            emitir!(self ; jmp =>passo);
        }
        let laco = self.lacos.pop().expect("laço recém-empilhado");
        resultado?;
        self.vincular(passo);
        self.terminado = false;
        if let Some(atualizacao) = atualizacao {
            self.instrucao(atualizacao)?;
        }
        if !self.terminado {
            emitir!(self ; jmp =>teste);
        }
        self.vincular(saida);
        // Laço sem teste e sem `break`: o que vem depois é inalcançável.
        self.terminado = !laco.saida_usada;
        Ok(())
    }

    /// Chama a função de impressão do runtime pelo endereço absoluto.
    ///
    /// `mov rax, QWORD <endereço>` seguido de `call rax` é a forma que não
    /// depende de relocação nem de onde o alocador decidiu pôr o código: o
    /// endereço é o da função Rust, que está na imagem do processo e não se
    /// move. A sequência é a mesma que a própria documentação do dynasm-rs
    /// recomenda para alcançar qualquer ponto do espaço de endereços de 64 bits.
    fn imprimir(&mut self, valor: Valor, span: Span) -> Result<(), Diagnostic> {
        let endereco = match valor.tipo {
            Tipo::Int => runtime::endereco_imprimir_i64(),
            Tipo::Bool => runtime::endereco_imprimir_bool(),
            Tipo::Void => return Err(erro(span, "impressão de void")),
        };
        let registrador = ARGUMENTOS[0];
        emitir!(self ; mov Rq(registrador), rax);
        emitir!(self ; mov rax, QWORD endereco);
        emitir!(self ; call rax);
        Ok(())
    }

    /// Traduz uma expressão da fatia, deixando o resultado em `RAX`.
    fn expressao(&mut self, expressao: &Expr<'_>) -> Result<Valor, Diagnostic> {
        match &expressao.kind {
            ExprKind::Int(valor) => {
                let literal = i64::from(*valor);
                emitir!(self ; mov rax, QWORD literal);
                Ok(Valor { tipo: Tipo::Int })
            }
            ExprKind::Bool(valor) => {
                let literal = i64::from(*valor);
                emitir!(self ; mov rax, QWORD literal);
                Ok(Valor { tipo: Tipo::Bool })
            }
            ExprKind::Identifier(nome) => {
                let (tipo_local, indice) = self.procurar(nome, expressao.span)?;
                self.carregar(indice);
                Ok(Valor { tipo: tipo_local })
            }
            ExprKind::Call { name, arguments } => self.chamada(name, arguments, expressao.span),
            ExprKind::Unary { op, operand } => {
                let valor = self.expressao(operand)?;
                match op {
                    UnaryOp::Negate => {
                        if valor.tipo != Tipo::Int {
                            return Err(Diagnostic::new(
                                "tipo incompatível na HIR asmjit",
                                expressao.span,
                            ));
                        }
                        // `neg` é `0 - x` em complemento de dois: o mesmo estouro
                        // modular que `sub i64 0, x` no backend LLVM.
                        emitir!(self ; neg rax);
                        Ok(Valor { tipo: Tipo::Int })
                    }
                    UnaryOp::Not => {
                        if valor.tipo != Tipo::Bool {
                            return Err(Diagnostic::new(
                                "tipo incompatível na HIR asmjit",
                                expressao.span,
                            ));
                        }
                        // O operando só assume 0 ou 1, então o xor com 1 nega.
                        emitir!(self ; xor rax, BYTE 1);
                        Ok(Valor { tipo: Tipo::Bool })
                    }
                    UnaryOp::NullAssert => Err(erro(expressao.span, "o operador `!` de não nulo")),
                    UnaryOp::BitNot => Err(erro(expressao.span, "operadores bit a bit")),
                }
            }
            ExprKind::Binary { op, left, right } => self.binaria(*op, left, right, expressao.span),
            _ => Err(erro(expressao.span, recurso_de_expressao(expressao))),
        }
    }

    /// Avalia `expressao` e derrama o resultado na posição temporária corrente.
    ///
    /// Devolve o índice da posição usada. A profundidade sobe enquanto o valor
    /// derramado estiver vivo e é restaurada por quem chamou.
    fn derramar(&mut self, expressao: &Expr<'_>) -> Result<(Valor, usize), Diagnostic> {
        let valor = self.expressao(expressao)?;
        let indice = self.base_temporarios + self.profundidade_atual;
        self.guardar(indice);
        self.profundidade_atual += 1;
        Ok((valor, indice))
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
            return Ok(Valor { tipo: Tipo::Void });
        }
        let assinatura = self
            .assinaturas
            .get(nome)
            .ok_or_else(|| Diagnostic::new("função não resolvida na HIR asmjit", span))?;
        if argumentos.len() != assinatura.parametros.len() {
            return Err(Diagnostic::new("aridade incorreta na HIR asmjit", span));
        }
        let rotulo = assinatura.rotulo;
        let resultado = assinatura.resultado;
        let esperados = assinatura.parametros.clone();

        let profundidade_inicial = self.profundidade_atual;
        let mut posicoes = Vec::with_capacity(argumentos.len());
        for (argumento, esperado) in argumentos.iter().zip(&esperados) {
            let (valor, indice) = self.derramar(argumento)?;
            if valor.tipo != *esperado {
                return Err(Diagnostic::new("tipo incompatível na HIR asmjit", span));
            }
            posicoes.push(indice);
        }
        self.profundidade_atual = profundidade_inicial;
        // Os argumentos só vão para os registradores depois que todos foram
        // avaliados: uma avaliação pode conter outra chamada, que destruiria
        // qualquer registrador de argumento já carregado.
        for (registrador, indice) in posicoes.iter().enumerate() {
            let posicao = deslocamento(*indice);
            let destino = ARGUMENTOS[registrador];
            emitir!(self ; mov Rq(destino), QWORD [rbp + posicao]);
        }
        emitir!(self ; call =>rotulo);
        Ok(Valor { tipo: resultado })
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
        let profundidade_inicial = self.profundidade_atual;
        let (valor_esquerdo, indice) = self.derramar(esquerda)?;
        let valor_direito = self.expressao(direita)?;
        self.profundidade_atual = profundidade_inicial;
        if valor_esquerdo.tipo != valor_direito.tipo || valor_esquerdo.tipo == Tipo::Void {
            return Err(Diagnostic::new("tipo incompatível na HIR asmjit", span));
        }
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
            return Err(Diagnostic::new("tipo incompatível na HIR asmjit", span));
        }
        // O direito está em RAX e o esquerdo na posição derramada; a ordem dos
        // operandos importa em `sub` e nas comparações, então o esquerdo volta
        // para RAX e o direito vai para RCX.
        let posicao = deslocamento(indice);
        emitir!(self ; mov rcx, rax);
        emitir!(self ; mov rax, QWORD [rbp + posicao]);
        // A condição de cada comparação é uma instrução diferente, e no
        // dynasm-rs a forma da instrução é fixada em tempo de compilação: não há
        // como parametrizar o sufixo. Cada comparação tem, por isso, seu próprio
        // braço — é o custo concreto de um montador de tempo de compilação.
        match op {
            // add/sub/imul são modulares em complemento de dois, exatamente como
            // add/sub/mul sem nsw/nuw do backend LLVM.
            BinaryOp::Add => {
                emitir!(self ; add rax, rcx);
                return Ok(Valor { tipo: Tipo::Int });
            }
            BinaryOp::Subtract => {
                emitir!(self ; sub rax, rcx);
                return Ok(Valor { tipo: Tipo::Int });
            }
            BinaryOp::Multiply => {
                emitir!(self ; imul rax, rcx);
                return Ok(Valor { tipo: Tipo::Int });
            }
            // `set` escreve um byte; o `movzx` zera os 56 bits restantes para
            // que a posição de 8 bytes guarde exatamente 0 ou 1.
            BinaryOp::Equal => {
                emitir!(self ; cmp rax, rcx);
                emitir!(self ; sete al);
            }
            BinaryOp::NotEqual => {
                emitir!(self ; cmp rax, rcx);
                emitir!(self ; setne al);
            }
            BinaryOp::Less => {
                emitir!(self ; cmp rax, rcx);
                emitir!(self ; setl al);
            }
            BinaryOp::LessEqual => {
                emitir!(self ; cmp rax, rcx);
                emitir!(self ; setle al);
            }
            BinaryOp::Greater => {
                emitir!(self ; cmp rax, rcx);
                emitir!(self ; setg al);
            }
            BinaryOp::GreaterEqual => {
                emitir!(self ; cmp rax, rcx);
                emitir!(self ; setge al);
            }
            BinaryOp::Remainder
            | BinaryOp::Divide
            | BinaryOp::TruncDivide
            | BinaryOp::IfNull
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
            | BinaryOp::ShiftRightUnsigned
            | BinaryOp::And
            | BinaryOp::Or => unreachable!("recusado ou tratado antes dos operandos"),
        }
        emitir!(self ; movzx rax, al);
        Ok(Valor { tipo: Tipo::Bool })
    }

    /// Emite `&&` e `||` avaliando o operando direito só quando necessário.
    fn curto_circuito(
        &mut self,
        op: BinaryOp,
        esquerda: &Expr<'_>,
        direita: &Expr<'_>,
        span: Span,
    ) -> Result<Valor, Diagnostic> {
        let valor = self.expressao(esquerda)?;
        if valor.tipo != Tipo::Bool {
            return Err(Diagnostic::new("tipo incompatível na HIR asmjit", span));
        }
        let curto = self.novo_rotulo();
        let fim = self.novo_rotulo();
        emitir!(self ; test rax, rax);
        // `&&` corta quando o esquerdo é falso; `||`, quando é verdadeiro.
        if op == BinaryOp::And {
            emitir!(self ; jz =>curto);
        } else {
            emitir!(self ; jnz =>curto);
        }
        let valor = self.expressao(direita)?;
        if valor.tipo != Tipo::Bool {
            return Err(Diagnostic::new("tipo incompatível na HIR asmjit", span));
        }
        emitir!(self ; jmp =>fim);
        self.vincular(curto);
        let constante = i64::from(op == BinaryOp::Or);
        emitir!(self ; mov rax, QWORD constante);
        self.vincular(fim);
        Ok(Valor { tipo: Tipo::Bool })
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
        | BinaryOp::ShiftRight
        | BinaryOp::ShiftRightUnsigned => Err(erro(span, "operadores bit a bit e deslocamentos")),
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
        ExprKind::List { .. }
        | ExprKind::Map { .. }
        | ExprKind::Set { .. }
        | ExprKind::MapEntry { .. }
        | ExprKind::Index { .. } => "coleções",
        ExprKind::Spread { .. } => "espalhamento em coleções",
        ExprKind::CollectionIf { .. } | ExprKind::CollectionFor { .. } => {
            "if e for em literais de coleção"
        }
        ExprKind::NullShort { .. } | ExprKind::NullShortTarget => "cadeias null-aware",
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
        // A célula de `late` — sentinela e checagem de inicialização — só é
        // emitida no backend JavaScript; aqui a declaração viraria `null` e ler
        // antes de escrever devolveria null em vez de lançar.
        StatementKind::Variable { is_late: true, .. } => Err(erro(instrucao.span, "late")),
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
            else_body.iter().flatten().try_for_each(validar_instrucao)
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
            initializer.as_deref().map_or(Ok(()), validar_instrucao)?;
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
