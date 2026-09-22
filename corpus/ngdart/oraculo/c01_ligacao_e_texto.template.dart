// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c01_ligacao_e_texto.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c01_ligacao_e_texto.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/src/runtime/interpolate.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$C01LigacaoETexto = const [];

class ViewC01LigacaoETexto0 extends import0.ComponentView<import1.C01LigacaoETexto> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  late final import3.DivElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewC01LigacaoETexto0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('c01-ligacao-e-texto'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/c01_ligacao_e_texto.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendDiv(doc, parentRenderNode);
    this._el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.escondido;
    if (import9.checkBinding(this._expr_0, currVal_0, 'escondido', 'package:corpus_ngdart/src/c01_ligacao_e_texto.html')) {
      import8.setProperty(this._el_0, 'hidden', currVal_0) /* REF:package:corpus_ngdart/src/c01_ligacao_e_texto.html:5:25 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import10.interpolateString0(_ctx.texto)) /* REF:package:corpus_ngdart/src/c01_ligacao_e_texto.html:26:35 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$C01LigacaoETexto, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C01LigacaoETextoNgFactory = ComponentFactory<import1.C01LigacaoETexto>('c01-ligacao-e-texto', viewFactory_C01LigacaoETextoHost0);
ComponentFactory<import1.C01LigacaoETexto> get C01LigacaoETextoNgFactory {
  return _C01LigacaoETextoNgFactory;
}

ComponentFactory<import1.C01LigacaoETexto> createC01LigacaoETextoFactory() {
  return ComponentFactory('c01-ligacao-e-texto', viewFactory_C01LigacaoETextoHost0);
}

final List<Object> styles$C01LigacaoETextoHost = const [];

class _ViewC01LigacaoETextoHost0 extends import12.HostView<import1.C01LigacaoETexto> {
  @override
  void build() {
    this.componentView = ViewC01LigacaoETexto0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C01LigacaoETexto();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.C01LigacaoETexto> viewFactory_C01LigacaoETextoHost0() {
  return _ViewC01LigacaoETextoHost0();
}
