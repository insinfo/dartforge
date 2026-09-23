# Classes e herança — subconjunto Dart 3.6.2

Classes nominais têm IDs estáveis dentro de uma unidade, campos tipados inicializados,
métodos com os parâmetros posicionais existentes e construtor implícito sem argumentos.
Tipos de classe podem ser anuláveis. extends permite uma base, inclusive declarada depois.
A análise rejeita ciclos e valida subtipagem, assinaturas e compatibilidade de overrides.

Acesso a membros no corpo de métodos exige this explícito. Métodos usam despacho dinâmico;
atribuições a campos avaliam o receptor antes do valor. Campos final não aceitam escrita.
Parâmetros e locais continuam em escopos distintos. Campos herdados não podem ser redeclarados
neste subconjunto e não há colisão entre campos e métodos.

Dart avalia inicializadores derivados antes dos da base. A emissão usa temporários antes
de super(), e só atribui a this depois; não usa ingenuamente campos nativos JavaScript.
Testes de três níveis verificam a ordem dos efeitos. Inicializadores não podem usar this.

Ainda não há construtores explícitos ou argumentos de construção, super explícito,
mixins, generics, static, factory, getters/setters ou tear-offs. Extensions têm um [subconjunto próprio](EXTENSIONS.md). Membros que dependem dos contratos de Object, como
toString/hashCode/runtimeType/noSuchMethod, são rejeitados. print(objeto) também é rejeitado
até haver semântica toString, e campos não são promovidos por testes de null.

## Bibliotecas e extensions

Classes agora podem vir de [bibliotecas relativas](MODULES.md). [Extensions](EXTENSIONS.md)
usam despacho estático e uma tabela de alvos na HIR; seu alcance entre bibliotecas ainda
não é suportado. Construtores explícitos e generics continuam no roteiro.

## Contratos abstratos e interfaces

`abstract class`, `interface class` e `abstract interface class` preservam origem
de biblioteca. `implements` admite múltiplos contratos e não herda corpos. Métodos
abstratos podem terminar em ponto e vírgula; classes concretas precisam fornecer
implementação compatível, própria ou herdada por `extends`. Classes abstratas não
podem ser instanciadas. Uma interface só pode ser estendida na sua biblioteca,
mas pode ser implementada fora dela.

Subtipagem percorre extends e implements. Assinaturas usam contravariância nos
parâmetros e covariância no resultado; um retorno concreto pode ser descartado
pelo contrato void. LLVM emite adaptadores conforme a assinatura estática do
receptor e despacha para a implementação da classe concreta. Propriedades de
interfaces ainda são rejeitadas explicitamente.

Enums simples têm identidade nominal e valores canônicos com name/index. Veja
[limites do subconjunto](SUBCONJUNTO.md) e [validação](IMPLEMENTACAO-11.md).
