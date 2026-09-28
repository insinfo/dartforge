// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j115_preserva_espacos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j115_preserva_espacos.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'dart:html' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/interpolate.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import18;

final List<Object> styles$J115Preserva = const [];

class ViewJ115Preserva0 extends import0.ComponentView<import1.J115Preserva> {
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  late final ViewContainer _appEl_5;
  late final NgIf _NgIf_5_9;
  Object? _expr_0;
  late final import5.HtmlElement _el_8;
  static import6.ComponentStyles? _componentStyles;
  ViewJ115Preserva0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import5.document.createElement('j115-preserva'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j115_preserva_espacos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import5.document;
    final _el_0 = import10.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'a');
    final _text_1 = import10.appendText(_el_0, '\n  ');
    final _el_2 = import10.appendSpan(doc, _el_0);
    _el_2.append(this._textBinding_3.element);
    final _text_4 = import10.appendText(_el_0, '\n   ');
    final _anchor_5 = import10.appendAnchor(_el_0);
    this._appEl_5 = ViewContainer(5, 0, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_J115Preserva1);
    this._NgIf_5_9 = NgIf(this._appEl_5, _TemplateRef_5_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_5, this._NgIf_5_9);
    }
    final _text_6 = import10.appendText(_el_0, '\n');
    final _text_7 = import10.appendText(parentRenderNode, '\n');
    this._el_8 = import10.appendElement<import5.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_9 = import10.appendText(this._el_8, '  fim  ');
    final _text_10 = import10.appendText(parentRenderNode, '\n');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_5_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_5_9.ngIf = _ctx.mostra /* REF:asset:corpus_ngdart/lib/src/j115_preserva_espacos.dart:349:363 */;
    this._appEl_5.detectChangesInNestedViews();
    this._textBinding_3.updateText(import13.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j115_preserva_espacos.dart:320:330 */;
    final currVal_0 = import13.interpolateString1('\n  ', _ctx.nome, '\n');
    if (import14.checkBinding(this._expr_0, currVal_0, '\n  {{ nome }}\n', 'asset:corpus_ngdart/lib/src/j115_preserva_espacos.dart')) {
      import10.setProperty(this._el_8, 'title', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j115_preserva_espacos.dart:380:402 */;
      this._expr_0 = currVal_0;
    }
  }

  @override
  void destroyInternal() {
    this._appEl_5.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J115Preserva, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J115PreservaNgFactory = ComponentFactory<import1.J115Preserva>('j115-preserva', viewFactory_J115PreservaHost0);
ComponentFactory<import1.J115Preserva> get J115PreservaNgFactory {
  return _J115PreservaNgFactory;
}

ComponentFactory<import1.J115Preserva> createJ115PreservaFactory() {
  return ComponentFactory('j115-preserva', viewFactory_J115PreservaHost0);
}

class _ViewJ115Preserva1 extends import16.EmbeddedView<import1.J115Preserva> {
  _ViewJ115Preserva1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('b'));
    final _text_1 = import10.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import16.EmbeddedView<void> viewFactory_J115Preserva1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ115Preserva1(parentView, parentIndex);
}

final List<Object> styles$J115PreservaHost = const [];

class _ViewJ115PreservaHost0 extends import18.HostView<import1.J115Preserva> {
  @override
  void build() {
    this.componentView = ViewJ115Preserva0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J115Preserva();
    this.initRootNode(_el_0);
  }
}

import18.HostView<import1.J115Preserva> viewFactory_J115PreservaHost0() {
  return _ViewJ115PreservaHost0();
}

final List<Object> styles$J115Minimiza = const [];

class ViewJ115Minimiza0 extends import0.ComponentView<import1.J115Minimiza> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  static import6.ComponentStyles? _componentStyles;
  ViewJ115Minimiza0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import5.document.createElement('j115-minimiza'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j115_preserva_espacos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import5.document;
    final _el_0 = import10.appendDiv(doc, parentRenderNode);
    final _el_1 = import10.appendSpan(doc, _el_0);
    _el_1.append(this._textBinding_2.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_2.updateText(import13.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j115_preserva_espacos.dart:610:620 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J115Minimiza, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J115MinimizaNgFactory = ComponentFactory<import1.J115Minimiza>('j115-minimiza', viewFactory_J115MinimizaHost0);
ComponentFactory<import1.J115Minimiza> get J115MinimizaNgFactory {
  return _J115MinimizaNgFactory;
}

ComponentFactory<import1.J115Minimiza> createJ115MinimizaFactory() {
  return ComponentFactory('j115-minimiza', viewFactory_J115MinimizaHost0);
}

final List<Object> styles$J115MinimizaHost = const [];

class _ViewJ115MinimizaHost0 extends import18.HostView<import1.J115Minimiza> {
  @override
  void build() {
    this.componentView = ViewJ115Minimiza0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J115Minimiza();
    this.initRootNode(_el_0);
  }
}

import18.HostView<import1.J115Minimiza> viewFactory_J115MinimizaHost0() {
  return _ViewJ115MinimizaHost0();
}
