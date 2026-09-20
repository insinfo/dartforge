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
interfaces/implements, mixins, generics, static, factory, getters/setters, tear-offs,
classes abstratas ou extensions. Membros que dependem dos contratos de Object, como
toString/hashCode/runtimeType/noSuchMethod, são rejeitados. print(objeto) também é rejeitado
até haver semântica toString, e campos não são promovidos por testes de null.

## Próxima etapa: extensions e bibliotecas

Extensions exigem resolução estática pelo tipo do receptor, e não despacho virtual JS.
Antes de emiti-las, a HIR deve carregar IDs dos membros resolvidos e o ambiente de imports.
A ordem planejada é resolução de bibliotecas/privacidade/prefixos, membros resolvidos na HIR,
depois extensions nomeadas sem generics sobre tipos explícitos, com testes de ambiguidade.
Nenhuma sintaxe de extension é aceita parcialmente ou convertida silenciosamente em método.
