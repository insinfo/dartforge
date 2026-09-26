import 'package:ngdart/angular.dart';

import 'i55_host_binding_formas.dart';

/// Sonda: filho com `@HostBinding`, na raiz e em `*ngIf`.
@Component(
  selector: 'i56-usa-host-binding',
  templateUrl: 'i56_usa_host_binding.html',
  directives: [coreDirectives, I55HostBindingFormas],
)
class I56UsaHostBinding {
  bool mostrar = true;
}
