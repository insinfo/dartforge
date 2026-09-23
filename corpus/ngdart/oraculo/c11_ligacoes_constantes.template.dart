// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c11_ligacoes_constantes.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c11_ligacoes_constantes.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$C11LigacoesConstantes = const [];

class ViewC11LigacoesConstantes0 extends import0.ComponentView<import1.C11LigacoesConstantes> {
  Object? _expr_1;
  late final import2.DivElement _el_0;
  late final import2.HtmlElement _el_1;
  late final import2.HtmlElement _el_2;
  static import3.ComponentStyles? _componentStyles;
  ViewC11LigacoesConstantes0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('c11-ligacoes-constantes'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/c11_ligacoes_constantes.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    this._el_1 = import7.appendSpan(doc, parentRenderNode);
    this._el_2 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'p');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      import7.setProperty(this._el_0, 'title', 'fixo') /* REF:package:corpus_ngdart/src/c11_ligacoes_constantes.html:5:21 */;
    }
    final currVal_1 = _ctx.escondido;
    if (import8.checkBinding(this._expr_1, currVal_1, 'escondido', 'package:corpus_ngdart/src/c11_ligacoes_constantes.html')) {
      import7.setProperty(this._el_0, 'hidden', currVal_1) /* REF:package:corpus_ngdart/src/c11_ligacoes_constantes.html:22:42 */;
      this._expr_1 = currVal_1;
    }
    if (firstCheck) {
      import7.setProperty(this._el_1, 'title', 'a') /* REF:package:corpus_ngdart/src/c11_ligacoes_constantes.html:55:68 */;
      import7.setProperty(this._el_2, 'title', 'b') /* REF:package:corpus_ngdart/src/c11_ligacoes_constantes.html:79:92 */;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$C11LigacoesConstantes, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C11LigacoesConstantesNgFactory = ComponentFactory<import1.C11LigacoesConstantes>('c11-ligacoes-constantes', viewFactory_C11LigacoesConstantesHost0);
ComponentFactory<import1.C11LigacoesConstantes> get C11LigacoesConstantesNgFactory {
  return _C11LigacoesConstantesNgFactory;
}

ComponentFactory<import1.C11LigacoesConstantes> createC11LigacoesConstantesFactory() {
  return ComponentFactory('c11-ligacoes-constantes', viewFactory_C11LigacoesConstantesHost0);
}

final List<Object> styles$C11LigacoesConstantesHost = const [];

class _ViewC11LigacoesConstantesHost0 extends import10.HostView<import1.C11LigacoesConstantes> {
  @override
  void build() {
    this.componentView = ViewC11LigacoesConstantes0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C11LigacoesConstantes();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.C11LigacoesConstantes> viewFactory_C11LigacoesConstantesHost0() {
  return _ViewC11LigacoesConstantesHost0();
}
