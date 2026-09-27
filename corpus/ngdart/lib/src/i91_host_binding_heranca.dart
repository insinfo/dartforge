import 'package:ngdart/angular.dart';

/// Base sem anotação de componente, com `@HostBinding`.
abstract class I91Base {
  @HostBinding('class.base')
  bool base = true;
  @HostBinding('attr.role')
  String get papel => 'b';
}

/// Sonda: `@HostBinding` herdado junto dos próprios.
@Component(
  selector: 'i91-host-binding-heranca',
  templateUrl: 'i91_host_binding_heranca.html',
)
class I91HostBindingHeranca extends I91Base {
  @HostBinding('class.proprio')
  bool proprio = false;
}
