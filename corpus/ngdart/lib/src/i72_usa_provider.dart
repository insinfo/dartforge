import 'package:ngdart/angular.dart';

import 'i60_provider_use_class.dart';

/// Sonda: filho com `providers:` de classe no template — os provedores dele
/// entram no nó de quem o usa (ainda recusado).
@Component(
  selector: 'i72-usa-provider',
  template: '<i60-provider-use-class></i60-provider-use-class>',
  directives: [I60ProviderUseClass],
)
class I72UsaProvider {}
