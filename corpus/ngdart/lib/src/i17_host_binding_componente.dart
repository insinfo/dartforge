import 'package:ngdart/angular.dart';

/// Sonda: `@HostBinding` em componente.
@Component(
  selector: 'i17-host-binding-componente',
  templateUrl: 'i17_host_binding_componente.html',
  directives: [coreDirectives],
)
class I17HostBindingComponente {
  @HostBinding('class.ativo')
  bool ativo = true;
  @HostBinding('attr.role')
  String papel = 'button';
}
