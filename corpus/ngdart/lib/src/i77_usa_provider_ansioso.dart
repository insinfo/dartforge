import 'package:ngdart/angular.dart';

import 'i66_provider_dependencias.dart';

/// Sonda de recusa: filho que injeta um provedor do próprio nó (o provedor
/// sai antes do filho, no `build()`).
@Component(
  selector: 'i77-usa-provider-ansioso',
  template: '<i66-provider-dependencias></i66-provider-dependencias>',
  directives: [I66ProviderDependencias],
)
class I77UsaProviderAnsioso {}
