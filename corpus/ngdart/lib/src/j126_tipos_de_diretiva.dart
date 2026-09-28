import 'package:ngdart/angular.dart';

/// `directiveTypes:` tipa os campos das diretivas genéricas da visão (a
/// visão e a instância do filho, o campo da diretiva), não a criação
/// (`lookupTypeArgumentsOf`, `compile_view.dart:1523-1569`). `#T` é o
/// parâmetro do componente; `on:` vale só no nó com aquele `#ref`.
@Component(
  selector: 'j126-item',
  template: '{{valor}}',
)
class J126Item<T> {
  @Input()
  T? valor;
}

@Directive(selector: '[j126Marca]')
class J126Marca<T> {
  @Input()
  T? marca;
}

@Component(
  selector: 'j126-lista',
  template: '''<j126-item [valor]="atual"></j126-item>
<div j126Marca #m [marca]="texto"></div>
<div j126Marca [marca]="texto"></div>''',
  directives: [J126Item, J126Marca],
  directiveTypes: [
    Typed<J126Item>.of([#T]),
    Typed<J126Marca<String>>(on: 'm'),
  ],
)
class J126Lista<T> {
  T? atual;
  String texto = 'a';
}

@Component(
  selector: 'j126-usa',
  template: '<j126-lista></j126-lista>',
  directives: [J126Lista],
  directiveTypes: [Typed<J126Lista<int>>()],
)
class J126Usa {}
