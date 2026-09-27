import 'package:ngdart/angular.dart';

class I75Servico {}

/// Sonda: componente com `providers:` que projeta conteúdo; o provedor vale
/// para o conteúdo (o `injectorGetInternal` cobre os nós projetados).
@Component(
  selector: 'i75-provider-projeta',
  template: '<ng-content></ng-content>',
  providers: [ClassProvider(I75Servico)],
)
class I75ProviderProjeta {}
