// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a18_interpolacao_nula.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a18_interpolacao_nula.dart' as import1;
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

final List<Object> styles$A18InterpolacaoNula = const [];

class ViewA18InterpolacaoNula0 extends import0.ComponentView<import1.A18InterpolacaoNula> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA18InterpolacaoNula0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a18-interpolacao-nula'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a18_interpolacao_nula.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
    final _el_2 = import8.appendDiv(doc, parentRenderNode);
    final _text_3 = import8.appendText(_el_2, import9.interpolate0(_ctx.fixo));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.talvez)) /* REF:package:corpus_ngdart/src/a18_interpolacao_nula.html:5:15 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A18InterpolacaoNula, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A18InterpolacaoNulaNgFactory = ComponentFactory<import1.A18InterpolacaoNula>('a18-interpolacao-nula', viewFactory_A18InterpolacaoNulaHost0);
ComponentFactory<import1.A18InterpolacaoNula> get A18InterpolacaoNulaNgFactory {
  return _A18InterpolacaoNulaNgFactory;
}

ComponentFactory<import1.A18InterpolacaoNula> createA18InterpolacaoNulaFactory() {
  return ComponentFactory('a18-interpolacao-nula', viewFactory_A18InterpolacaoNulaHost0);
}

final List<Object> styles$A18InterpolacaoNulaHost = const [];

class _ViewA18InterpolacaoNulaHost0 extends import11.HostView<import1.A18InterpolacaoNula> {
  @override
  void build() {
    this.componentView = ViewA18InterpolacaoNula0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A18InterpolacaoNula();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A18InterpolacaoNula> viewFactory_A18InterpolacaoNulaHost0() {
  return _ViewA18InterpolacaoNulaHost0();
}
