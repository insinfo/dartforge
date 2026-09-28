import 'package:ngdart/angular.dart';

/// `@HostBinding` em membros estáticos e em getter com outra anotação (o
/// `material-checkbox`, o `material-dialog` e o `material-button` do
/// ngcomponents).
@Component(
  selector: 'j96-host-binding-estatico',
  template: '<i [attr.tabindex]="indiceTexto"></i>',
)
class J96HostBindingEstatico {
  J96HostBindingEstatico(@Attribute('role') String? role)
      : role = role ?? 'checkbox';

  @HostBinding('class')
  static const hostClass = 'themeable';

  @HostBinding('attr.aria-modal')
  static const ariaModal = 'true';

  @HostBinding('attr.tabindex')
  static const tabIndex = -1;

  @HostBinding('attr.role')
  final String role;

  @HostBinding('attr.disabled')
  @visibleForTemplate
  String? get hostDisabled => disabled ? '' : null;

  bool disabled = false;

  String indiceTexto = '0';

  @HostBinding('attr.tabIndex')
  String get indiceDoHospedeiro => '1';
}

@Component(
  selector: 'j96-usa',
  template: '<j96-host-binding-estatico></j96-host-binding-estatico>',
  directives: [J96HostBindingEstatico],
)
class J96Usa {}
