// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c06_interpolacao_em_cadeia.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c06_interpolacao_em_cadeia.dart' as import1;
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

final List<Object> styles$C06InterpolacaoEmCadeia = const [];

class ViewC06InterpolacaoEmCadeia0 extends import0.ComponentView<import1.C06InterpolacaoEmCadeia> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewC06InterpolacaoEmCadeia0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('c06-interpolacao-em-cadeia'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/c06_interpolacao_em_cadeia.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
    final _el_2 = import8.appendDiv(doc, parentRenderNode);
    _el_2.append(this._textBinding_3.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.item.nome)) /* REF:package:corpus_ngdart/src/c06_interpolacao_em_cadeia.html:5:18 */;
    this._textBinding_3.updateTextWithPrimitive(_ctx.item.quantidade) /* REF:package:corpus_ngdart/src/c06_interpolacao_em_cadeia.html:29:48 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$C06InterpolacaoEmCadeia, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C06InterpolacaoEmCadeiaNgFactory = ComponentFactory<import1.C06InterpolacaoEmCadeia>('c06-interpolacao-em-cadeia', viewFactory_C06InterpolacaoEmCadeiaHost0);
ComponentFactory<import1.C06InterpolacaoEmCadeia> get C06InterpolacaoEmCadeiaNgFactory {
  return _C06InterpolacaoEmCadeiaNgFactory;
}

ComponentFactory<import1.C06InterpolacaoEmCadeia> createC06InterpolacaoEmCadeiaFactory() {
  return ComponentFactory('c06-interpolacao-em-cadeia', viewFactory_C06InterpolacaoEmCadeiaHost0);
}

final List<Object> styles$C06InterpolacaoEmCadeiaHost = const [];

class _ViewC06InterpolacaoEmCadeiaHost0 extends import11.HostView<import1.C06InterpolacaoEmCadeia> {
  @override
  void build() {
    this.componentView = ViewC06InterpolacaoEmCadeia0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C06InterpolacaoEmCadeia();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.C06InterpolacaoEmCadeia> viewFactory_C06InterpolacaoEmCadeiaHost0() {
  return _ViewC06InterpolacaoEmCadeiaHost0();
}
