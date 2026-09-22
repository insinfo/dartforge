// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a04_interpolacao_com_texto.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a04_interpolacao_com_texto.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$A04InterpolacaoComTexto = const [];

class ViewA04InterpolacaoComTexto0 extends import0.ComponentView<import1.A04InterpolacaoComTexto> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA04InterpolacaoComTexto0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a04-interpolacao-com-texto'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a04_interpolacao_com_texto.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    final _text_1 = import8.appendText(_el_0, 'Ola ');
    _el_0.append(this._textBinding_2.element);
    final _text_3 = import8.appendText(_el_0, '!');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_2.updateText(import9.interpolateString0(_ctx.nome)) /* REF:package:corpus_ngdart/src/a04_interpolacao_com_texto.html:9:17 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A04InterpolacaoComTexto, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A04InterpolacaoComTextoNgFactory = ComponentFactory<import1.A04InterpolacaoComTexto>('a04-interpolacao-com-texto', viewFactory_A04InterpolacaoComTextoHost0);
ComponentFactory<import1.A04InterpolacaoComTexto> get A04InterpolacaoComTextoNgFactory {
  return _A04InterpolacaoComTextoNgFactory;
}

ComponentFactory<import1.A04InterpolacaoComTexto> createA04InterpolacaoComTextoFactory() {
  return ComponentFactory('a04-interpolacao-com-texto', viewFactory_A04InterpolacaoComTextoHost0);
}

final List<Object> styles$A04InterpolacaoComTextoHost = const [];

class _ViewA04InterpolacaoComTextoHost0 extends import11.HostView<import1.A04InterpolacaoComTexto> {
  @override
  void build() {
    this.componentView = ViewA04InterpolacaoComTexto0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A04InterpolacaoComTexto();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A04InterpolacaoComTexto> viewFactory_A04InterpolacaoComTextoHost0() {
  return _ViewA04InterpolacaoComTextoHost0();
}
