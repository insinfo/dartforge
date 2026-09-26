// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i13_seguro_nulo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i13_seguro_nulo.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I13SeguroNulo = const [];

class ViewI13SeguroNulo0 extends import0.ComponentView<import1.I13SeguroNulo> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  late final import3.HtmlElement _el_2;
  static import4.ComponentStyles? _componentStyles;
  ViewI13SeguroNulo0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('i13-seguro-nulo'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i13_seguro_nulo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    final _el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    this._el_2 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.pessoa?.nome)) /* REF:package:corpus_ngdart/src/i13_seguro_nulo.html:3:19 */;
    final currVal_0 = (_ctx.pessoa?.nome ?? '-');
    if (import10.checkBinding(this._expr_0, currVal_0, 'pessoa?.nome ?? \'-\'', 'package:corpus_ngdart/src/i13_seguro_nulo.html')) {
      import8.setProperty(this._el_2, 'title', currVal_0) /* REF:package:corpus_ngdart/src/i13_seguro_nulo.html:26:55 */;
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I13SeguroNulo, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I13SeguroNuloNgFactory = ComponentFactory<import1.I13SeguroNulo>('i13-seguro-nulo', viewFactory_I13SeguroNuloHost0);
ComponentFactory<import1.I13SeguroNulo> get I13SeguroNuloNgFactory {
  return _I13SeguroNuloNgFactory;
}

ComponentFactory<import1.I13SeguroNulo> createI13SeguroNuloFactory() {
  return ComponentFactory('i13-seguro-nulo', viewFactory_I13SeguroNuloHost0);
}

final List<Object> styles$I13SeguroNuloHost = const [];

class _ViewI13SeguroNuloHost0 extends import12.HostView<import1.I13SeguroNulo> {
  @override
  void build() {
    this.componentView = ViewI13SeguroNulo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I13SeguroNulo();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I13SeguroNulo> viewFactory_I13SeguroNuloHost0() {
  return _ViewI13SeguroNuloHost0();
}
