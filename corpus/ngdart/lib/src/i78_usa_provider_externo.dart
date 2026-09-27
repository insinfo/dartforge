import 'package:ngdart/angular.dart';

import 'i67_provider_externo.dart';

/// Sonda de recusa: filho com provedor que depende de algo de fora do nó
/// (viria dos elementos acima ou do injetor de fora).
@Component(
  selector: 'i78-usa-provider-externo',
  template: '<i67-provider-externo></i67-provider-externo>',
  directives: [I67ProviderExterno],
)
class I78UsaProviderExterno {}
