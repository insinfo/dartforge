// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a03_interpolacao_int.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a03_interpolacao_int.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$A03InterpolacaoInt = const [];

class ViewA03InterpolacaoInt0 extends import0.ComponentView<import1.A03InterpolacaoInt> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA03InterpolacaoInt0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a03-interpolacao-int'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a03_interpolacao_int.dart' : null);
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
    this._textBinding_1.updateTextWithPrimitive(_ctx.quantidade) /* REF:package:corpus_ngdart/src/a03_interpolacao_int.html:5:19 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A03InterpolacaoInt, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A03InterpolacaoIntNgFactory = ComponentFactory<import1.A03InterpolacaoInt>('a03-interpolacao-int', viewFactory_A03InterpolacaoIntHost0);
ComponentFactory<import1.A03InterpolacaoInt> get A03InterpolacaoIntNgFactory {
  return _A03InterpolacaoIntNgFactory;
}

ComponentFactory<import1.A03InterpolacaoInt> createA03InterpolacaoIntFactory() {
  return ComponentFactory('a03-interpolacao-int', viewFactory_A03InterpolacaoIntHost0);
}

final List<Object> styles$A03InterpolacaoIntHost = const [];

class _ViewA03InterpolacaoIntHost0 extends import10.HostView<import1.A03InterpolacaoInt> {
  @override
  void build() {
    this.componentView = ViewA03InterpolacaoInt0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A03InterpolacaoInt();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.A03InterpolacaoInt> viewFactory_A03InterpolacaoIntHost0() {
  return _ViewA03InterpolacaoIntHost0();
}
