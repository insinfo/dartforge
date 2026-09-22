// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c03_dois_elementos_ligados.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c03_dois_elementos_ligados.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$C03DoisElementosLigados = const [];

class ViewC03DoisElementosLigados0 extends import0.ComponentView<import1.C03DoisElementosLigados> {
  Object? _expr_0;
  Object? _expr_1;
  late final import2.DivElement _el_0;
  late final import2.HtmlElement _el_1;
  static import3.ComponentStyles? _componentStyles;
  ViewC03DoisElementosLigados0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('c03-dois-elementos-ligados'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/c03_dois_elementos_ligados.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    this._el_1 = import7.appendSpan(doc, this._el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.a;
    if (import8.checkBinding(this._expr_0, currVal_0, 'a', 'package:corpus_ngdart/src/c03_dois_elementos_ligados.html')) {
      import7.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/c03_dois_elementos_ligados.html:5:16 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.b;
    if (import8.checkBinding(this._expr_1, currVal_1, 'b', 'package:corpus_ngdart/src/c03_dois_elementos_ligados.html')) {
      import7.setProperty(this._el_1, 'title', currVal_1) /* REF:package:corpus_ngdart/src/c03_dois_elementos_ligados.html:23:34 */;
      this._expr_1 = currVal_1;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$C03DoisElementosLigados, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C03DoisElementosLigadosNgFactory = ComponentFactory<import1.C03DoisElementosLigados>('c03-dois-elementos-ligados', viewFactory_C03DoisElementosLigadosHost0);
ComponentFactory<import1.C03DoisElementosLigados> get C03DoisElementosLigadosNgFactory {
  return _C03DoisElementosLigadosNgFactory;
}

ComponentFactory<import1.C03DoisElementosLigados> createC03DoisElementosLigadosFactory() {
  return ComponentFactory('c03-dois-elementos-ligados', viewFactory_C03DoisElementosLigadosHost0);
}

final List<Object> styles$C03DoisElementosLigadosHost = const [];

class _ViewC03DoisElementosLigadosHost0 extends import10.HostView<import1.C03DoisElementosLigados> {
  @override
  void build() {
    this.componentView = ViewC03DoisElementosLigados0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C03DoisElementosLigados();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.C03DoisElementosLigados> viewFactory_C03DoisElementosLigadosHost0() {
  return _ViewC03DoisElementosLigadosHost0();
}
