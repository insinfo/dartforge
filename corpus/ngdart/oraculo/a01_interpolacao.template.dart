// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a01_interpolacao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a01_interpolacao.dart' as import1;
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

final List<Object> styles$A01Interpolacao = const [];

class ViewA01Interpolacao0 extends import0.ComponentView<import1.A01Interpolacao> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA01Interpolacao0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a01-interpolacao'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a01_interpolacao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.mensagem)) /* REF:package:corpus_ngdart/src/a01_interpolacao.html:5:17 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A01Interpolacao, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A01InterpolacaoNgFactory = ComponentFactory<import1.A01Interpolacao>('a01-interpolacao', viewFactory_A01InterpolacaoHost0);
ComponentFactory<import1.A01Interpolacao> get A01InterpolacaoNgFactory {
  return _A01InterpolacaoNgFactory;
}

ComponentFactory<import1.A01Interpolacao> createA01InterpolacaoFactory() {
  return ComponentFactory('a01-interpolacao', viewFactory_A01InterpolacaoHost0);
}

final List<Object> styles$A01InterpolacaoHost = const [];

class _ViewA01InterpolacaoHost0 extends import11.HostView<import1.A01Interpolacao> {
  @override
  void build() {
    this.componentView = ViewA01Interpolacao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A01Interpolacao();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A01Interpolacao> viewFactory_A01InterpolacaoHost0() {
  return _ViewA01InterpolacaoHost0();
}
