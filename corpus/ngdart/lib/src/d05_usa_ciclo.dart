import 'package:ngdart/angular.dart';

import 'd05_filho_ciclo.dart';

/// Filho com ciclo de vida: `changed` e `ngAfterChanges`, `ngOnInit` na
/// primeira checagem, `ngDoCheck`, os ganchos de conteúdo antes das
/// ligações de texto, os de visão depois da visão-filha e `ngOnDestroy` no
/// fim do `destroyInternal`. O atributo estático entra como entrada literal
/// no `if (firstCheck)`, antes da dinâmica.
@Component(
  selector: 'd05-usa-ciclo',
  templateUrl: 'd05_usa_ciclo.html',
  directives: [D05FilhoCiclo],
)
class D05UsaCiclo {
  int total = 1;
}
