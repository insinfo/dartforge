import 'package:ngdart/angular.dart';

const j97Relogio = OpaqueToken('j97Relogio');
const OpaqueToken<String> j97Tipado = OpaqueToken('j97Tipado');

/// `OpaqueToken('x')` sem argumento de tipo (o `datepickerClock` do
/// ngcomponents: `OpaqueToken<Object>`), o tipo vindo da declaração, e
/// componente genérico com `@HostBinding`.
@Component(
  selector: 'j97-token-sem-tipo',
  template: '<i></i><p *ngIf="compacto">{{ valor }}</p>',
  directives: [NgIf],
)
class J97TokenSemTipo<T> {
  J97TokenSemTipo(
    @Optional() @Inject(j97Relogio) Object? relogio,
    @Optional() @Inject(j97Tipado) String? nome,
    @Attribute('modo') String? modo,
  );

  @HostBinding('class.compacto')
  bool compacto = false;

  T? valor;
}

@Component(
  selector: 'j97-limitado',
  template: '<b>{{ total }}</b>',
)
class J97Limitado<T extends num, U> {
  T? total;
  U? outro;
}

@Component(
  selector: 'j97-usa',
  template: '<j97-token-sem-tipo modo="x"></j97-token-sem-tipo><j97-limitado></j97-limitado>',
  directives: [J97TokenSemTipo, J97Limitado],
)
class J97Usa {}
