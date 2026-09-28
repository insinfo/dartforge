import 'package:ngdart/angular.dart';

/// Diretiva estrutural própria (a `deferredContent` do ngcomponents).
@Directive(selector: '[j94Adiado]')
class J94Adiado implements OnInit {
  J94Adiado(this._container, this._modelo);

  final ViewContainerRef _container;
  final TemplateRef _modelo;

  @Input('j94Adiado')
  bool preservar = false;

  @Input()
  set j94AdiadoForcar(bool v) {}

  @override
  void ngOnInit() {
    _container.createEmbeddedView(_modelo);
  }
}

/// Repete o conteúdo com um local (`let x of`, como o `ngFor`).
@Directive(selector: '[j94Repetir]')
class J94Repetir {
  J94Repetir(this._container, this._modelo);

  final ViewContainerRef _container;
  final TemplateRef _modelo;

  @Input()
  set j94RepetirDe(List<String> itens) {
    _container.clear();
    for (final i in itens) {
      _container.createEmbeddedView(_modelo).setLocal(r'$implicit', i);
    }
  }
}

/// `*x` de diretivas estruturais que não são as do ngdart.
@Component(
  selector: 'j94-estrutural-propria',
  template: '<div *j94Adiado><b>{{ texto }}</b></div>'
      '<p *j94Adiado="true; forcar: ligado">x</p>'
      '<i *j94Repetir="let s de itens">{{ s }}</i>',
  directives: [J94Adiado, J94Repetir],
)
class J94EstruturalPropria {
  String texto = 't';
  bool ligado = false;
  List<String> itens = ['a'];
}
