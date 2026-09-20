# Genéricos, constantes e enums avançadas

Alvo: Dart 3.6.2. Este incremento amplia o backend JavaScript. LLVM diagnostica
funções genéricas, enums avançadas e switches como recursos ainda não implementados.
Constantes escalares locais continuam utilizáveis no subconjunto nativo.

## Funções genéricas

Funções top-level aceitam parâmetros de tipo, argumentos explícitos e inferência
pelos argumentos da chamada, inclusive em List, Iterable e assinaturas funcionais.
Desde o incremento 18, o corpo é verificado sob os limites dos parâmetros e a
emissão JavaScript preserva descritores para testes/casts reificados. O limite
padrão é Object?. Não há classes ou métodos genéricos do usuário nem tear-offs
genéricos. A inferência permanece parcial; veja
[bounds e reificação](GENERICS-REIFIED-REFERENCIAS.md).

## Constantes

Declarações locais const e listas em contexto const são avaliadas antes da emissão.
Listas constantes são transitivamente imutáveis e canônicas: a chave considera
valores e tipos estruturais, inclusive através de imports. Listas mutáveis não
compartilham essa identidade. Mutação por índice ou add falha em execução.

O avaliador aceita escalares, referências a const locais, valores de enum,
operadores suportados e listas aninhadas. Valida também a sintaxe dos operandos
não executados pelo curto-circuito. Inteiros constantes têm o limite i32 atual do
frontend; overflow é diagnosticado. Constantes top-level, construtores const de
classes do usuário e execução de getters em contexto const permanecem pendentes.
A fusão de funções fica conservadoramente desativada neste subconjunto quando
há constantes avaliadas ou funções genéricas; não altera identidades observáveis.

## Enums e padrões

Enums avançadas aceitam campos finais escalares (int, bool, String e suas formas
anuláveis), construtor const com parâmetros this.campo, métodos e getters.
Interfaces de métodos seguem os contratos já existentes. Construtores nomeados,
initializers, campos de coleção e instâncias const arbitrárias ficam fora do escopo.
Os valores são singletons imutáveis, com index e name.

Switch aceita expressão ou statement, constantes, wildcard e bindings tipados
escalares/nominais com guardas when. O discriminante é avaliado uma vez; guardas
executam na ordem dos casos. Break encerra o switch e continue preserva o laço
externo. Não há fallthrough implícito. Padrões de objeto, lista, record e
combinações lógicas ainda não são suportados.

Expressões switch exigem exaustividade. Statements sobre domínios finitos enum e
bool também são verificados. Um caso guardado não fornece cobertura, mesmo com
when true: o exemplo de prioridades com guarda nivel >= 3 precisa de casos
adicionais ou default para cobrir alta e critica estaticamente.

Interpolação de strings ainda não está implementada; os fixtures usam concatenação.

## Referências e regressões

A semântica foi conferida no SDK Dart 3.6.2, tag local b0cc5495e0f5e8ae150825a5352e708cb49e65ff,
em sdk/lib/core/enum.dart e nas suítes de enums, constantes, genéricos e padrões.
Os testes próprios incluem execução contra o Dart 3.6.2, identidade de listas
aninhadas entre bibliotecas, getters, guardas com efeitos e controle de laços.
Veja [incremento 13](IMPLEMENTACAO-13.md) para os resultados integrados.
