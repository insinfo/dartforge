// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a05_duas_interpolacoes.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a05_duas_interpolacoes.dart' as import1;
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

final List<Object> styles$A05DuasInterpolacoes = const [];

class ViewA05DuasInterpolacoes0 extends import0.ComponentView<import1.A05DuasInterpolacoes> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA05DuasInterpolacoes0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a05-duas-interpolacoes'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a05_duas_interpolacoes.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import8.appendText(_el_0, ' e ');
    _el_0.append(this._textBinding_3.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.a)) /* REF:package:corpus_ngdart/src/a05_duas_interpolacoes.html:5:10 */;
    this._textBinding_3.updateText(import9.interpolateString0(_ctx.b)) /* REF:package:corpus_ngdart/src/a05_duas_interpolacoes.html:13:18 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A05DuasInterpolacoes, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A05DuasInterpolacoesNgFactory = ComponentFactory<import1.A05DuasInterpolacoes>('a05-duas-interpolacoes', viewFactory_A05DuasInterpolacoesHost0);
ComponentFactory<import1.A05DuasInterpolacoes> get A05DuasInterpolacoesNgFactory {
  return _A05DuasInterpolacoesNgFactory;
}

ComponentFactory<import1.A05DuasInterpolacoes> createA05DuasInterpolacoesFactory() {
  return ComponentFactory('a05-duas-interpolacoes', viewFactory_A05DuasInterpolacoesHost0);
}

final List<Object> styles$A05DuasInterpolacoesHost = const [];

class _ViewA05DuasInterpolacoesHost0 extends import11.HostView<import1.A05DuasInterpolacoes> {
  @override
  void build() {
    this.componentView = ViewA05DuasInterpolacoes0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A05DuasInterpolacoes();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A05DuasInterpolacoes> viewFactory_A05DuasInterpolacoesHost0() {
  return _ViewA05DuasInterpolacoesHost0();
}
