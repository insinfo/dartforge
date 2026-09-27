import 'package:ngdart/angular.dart';

import 'i69_marca.dart';

class I69Servico {}

class I69Outro {}

/// Sonda: `providers:` junto de `@ContentChildren` e de gancho de ciclo de
/// vida, com o componente dependendo de um provedor.
@Component(
  selector: 'i69-providers-e-consulta',
  template: '<ng-content></ng-content>',
  providers: [ClassProvider(I69Servico), ClassProvider(I69Outro)],
)
class I69ProvidersEConsulta implements OnInit {
  final I69Servico servico;
  I69ProvidersEConsulta(this.servico);

  @ContentChildren(I69Marca)
  List<I69Marca>? marcas;

  @override
  void ngOnInit() {}
}
