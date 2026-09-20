# Null safety e fluxo — subconjunto Dart 3.6.2

Tipos int?, String?, bool? e classes anuláveis aceitam null. O tipo declarado permanece
separado da promoção de fluxo: promover uma variável não muda os valores que ela pode
receber em atribuições posteriores. Parâmetros e variáveis locais são analisados por escopo.

Comparações == null e != null refinam ramos de if, e &&/|| consideram curto-circuito.
Retornos antecipados podem preservar promoções no caminho restante. Atribuições
atualizam o estado; junções de caminhos e laços descartam fatos que não são seguros.
A análise de laços é conservadora e pode rejeitar programas válidos no Dart completo.
Campos não são promovidos neste incremento.

O operador ?? avalia o lado direito somente quando necessário. A asserção ! avalia seu
operando uma vez e lança TypeError JavaScript se ele for null; a hierarquia completa
de erros de Dart ainda não foi implementada. Funções de retorno anulável que terminam
sem return produzem null, e return vazio nesse contexto é rejeitado.

Ainda não há dynamic, Never, late, definite assignment de locais sem inicializador,
is/as ou promoção geral por subtipos. var x = null exige dynamic e é rejeitado; declare
um tipo anulável explicitamente. A asserção sobre um literal null também é rejeitada.
