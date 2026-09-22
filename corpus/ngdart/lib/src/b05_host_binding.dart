import 'package:ngdart/angular.dart';

/// `@HostBinding` — ligação na própria raiz do componente.
@Component(
  selector: 'b05-host-binding',
  templateUrl: 'b05_host_binding.html',
)
class B05HostBinding {
  @HostBinding('class.ativo')
  bool ativo = true;
}
