# Modificadores de classe, mixins e sealed

Alvo: Dart 3.6.2. O compilador distingue modificador nominal e forma da declaração:
class, mixin ou mixin class. abstract e interface mantêm seus contratos anteriores.

As restrições valem por **biblioteca**, não simplesmente por arquivo. O carregador
atual trata cada arquivo importado como biblioteca e ainda rejeita part/part of.
`final class` restringe subtipagem externa; não torna automaticamente seus campos
ou instâncias imutáveis. `implements` representa subtipagem nominal em Dart.

## Fronteiras e propagação

- interface permite implements externo e impede extends externo.
- base permite extends externo e impede implements externo, inclusive quando
  uma interface intermediária expõe indiretamente o contrato base.
- final impede extends e implements externos.
- sealed é implicitamente abstrata e restringe subtipos diretos à mesma biblioteca.
- Subtipos de base/final precisam de base, final ou sealed, inclusive dentro da
  biblioteca. A restrição se propaga através de tipos intermediários sealed.
- abstract e mixin isolado não podem ser instanciados; mixin isolado não admite extends.

Enums continuam finais implicitamente. Formas suportadas incluem abstract base,
abstract final, abstract interface, base mixin, mixin class e abstract/base mixin class.
Combinações incompatíveis recebem diagnóstico.

## Aplicação de mixins

`C extends B with M, N` é expandida em aplicações abstratas internas na ordem
B -> aplicação M -> aplicação N -> C. A implementação mais à direita prevalece,
sujeita às verificações de override. Campos preservam a ordem de inicialização,
identidade por instância e nomes privados da biblioteca de origem.

O passe ocorre após resolver imports e antes da análise semântica. Cada aplicação
mantém uma relação nominal com o mixin original; implements não copia código.
Corpos clonados conservam a resolução estática da declaração original. O processo
reutiliza herança, layouts e despacho dos backends, sem concatenar código fonte.

Neste incremento, with aceita mixin e mixin class em classes. Restrições on,
chamadas super, construtores explícitos, aplicações em enums e aliases de aplicação
nomeados ainda não são implementados. Conflitos de campos seguem as restrições
atuais do subconjunto, em vez de simular toda a semântica de getters/setters Dart.

## Exaustividade

Padrões de objeto vazios `Autenticado()` verificam o tipo sem criar uma instância.
Desestruturação de propriedades ainda não é suportada. A cobertura de sealed
expande seus subtipos diretos fechados; uma subclasse aberta exige um padrão que
cubra o próprio tipo. Conhecer algumas subclasses concretas de um tipo aberto
não autoriza considerar o switch exaustivo. Guardas continuam sem fornecer cobertura;
a forma anulável também exige cobertura de null.

Switch e padrões executam no JavaScript. O backend LLVM mantém diagnóstico explícito
para switch; os modificadores e o subconjunto de mixins sem esses recursos usam
a infraestrutura nativa de classes existente.

## Referências

SDK fixado na tag 3.6.2 (b0cc5495e0f5e8ae150825a5352e708cb49e65ff), especialmente
as suítes tests/language/class_modifiers, base_transitivity e testes de patterns.
Os fixtures próprios são comparados com Dart 3.6.2. Exemplo:
[EstadoLogin, Voador e Pagamento](../../examples/modifiers/main.dart).
