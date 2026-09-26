import 'package:ngdart/angular.dart';

/// Sonda: `@HostBinding` de classe e atributo (campo, `final` e getter) com ligações no template.
@Component(
  selector: 'i55-host-binding-formas',
  templateUrl: 'i55_host_binding_formas.html',
  directives: [],
)
class I55HostBindingFormas {
  int n = 1;
  @HostBinding('class.ativo')
  bool ativo = true;
  @HostBinding('attr.role')
  String papel = 'button';
  @HostBinding('class.fixo')
  final bool fixo = true;
  @HostBinding('attr.aria-label')
  String get rotulo => 'r';
}
