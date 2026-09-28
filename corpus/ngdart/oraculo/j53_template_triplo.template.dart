// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j53_template_triplo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j53_template_triplo.dart' as import1;
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

final List<Object> styles$J53ComQuebra = const [];

class ViewJ53ComQuebra0 extends import0.ComponentView<import1.J53ComQuebra> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  late final import3.HtmlElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewJ53ComQuebra0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j53-com-quebra'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j53_template_triplo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendSpan(doc, parentRenderNode);
    this._el_0.append(this._textBinding_1.element);
    final _text_2 = import8.appendText(parentRenderNode, '\n');
    final _el_3 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_4 = import8.appendText(_el_3, 'b');
    _el_3.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.nome;
    if (import9.checkBinding(this._expr_0, currVal_0, 'nome', 'asset:corpus_ngdart/lib/src/j53_template_triplo.dart')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j53_template_triplo.dart:301:315 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import10.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j53_template_triplo.dart:316:326 */;
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.nome = 'x';
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J53ComQuebra, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J53ComQuebraNgFactory = ComponentFactory<import1.J53ComQuebra>('j53-com-quebra', viewFactory_J53ComQuebraHost0);
ComponentFactory<import1.J53ComQuebra> get J53ComQuebraNgFactory {
  return _J53ComQuebraNgFactory;
}

ComponentFactory<import1.J53ComQuebra> createJ53ComQuebraFactory() {
  return ComponentFactory('j53-com-quebra', viewFactory_J53ComQuebraHost0);
}

final List<Object> styles$J53ComQuebraHost = const [];

class _ViewJ53ComQuebraHost0 extends import12.HostView<import1.J53ComQuebra> {
  @override
  void build() {
    this.componentView = ViewJ53ComQuebra0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J53ComQuebra();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J53ComQuebra> viewFactory_J53ComQuebraHost0() {
  return _ViewJ53ComQuebraHost0();
}

final List<Object> styles$J53SemQuebra = const [];

class ViewJ53SemQuebra0 extends import0.ComponentView<import1.J53SemQuebra> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  Object? _expr_0;
  late final import3.HtmlElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewJ53SemQuebra0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j53-sem-quebra'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j53_template_triplo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'i');
    this._el_0.append(this._textBinding_1.element);
    final _el_2 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'u');
    _el_2.append(this._textBinding_3.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.nome;
    if (import9.checkBinding(this._expr_0, currVal_0, 'nome', 'asset:corpus_ngdart/lib/src/j53_template_triplo.dart')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j53_template_triplo.dart:476:490 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import10.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j53_template_triplo.dart:491:499 */;
    this._textBinding_3.updateText(import10.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j53_template_triplo.dart:507:517 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J53SemQuebra, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J53SemQuebraNgFactory = ComponentFactory<import1.J53SemQuebra>('j53-sem-quebra', viewFactory_J53SemQuebraHost0);
ComponentFactory<import1.J53SemQuebra> get J53SemQuebraNgFactory {
  return _J53SemQuebraNgFactory;
}

ComponentFactory<import1.J53SemQuebra> createJ53SemQuebraFactory() {
  return ComponentFactory('j53-sem-quebra', viewFactory_J53SemQuebraHost0);
}

final List<Object> styles$J53SemQuebraHost = const [];

class _ViewJ53SemQuebraHost0 extends import12.HostView<import1.J53SemQuebra> {
  @override
  void build() {
    this.componentView = ViewJ53SemQuebra0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J53SemQuebra();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J53SemQuebra> viewFactory_J53SemQuebraHost0() {
  return _ViewJ53SemQuebraHost0();
}
