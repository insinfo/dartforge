// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a25_local_ancestral.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a25_local_ancestral.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/runtime/text_binding.dart' as import16;
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$A25LocalAncestral = const [];

class ViewA25LocalAncestral0 extends import0.ComponentView<import1.A25LocalAncestral> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewA25LocalAncestral0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('a25-local-ancestral'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/a25_local_ancestral.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_A25LocalAncestral1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.grupos;
    if (import12.checkBinding(this._expr_0, currVal_0, 'grupos', 'package:corpus_ngdart/src/a25_local_ancestral.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/a25_local_ancestral.html:5:44 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$A25LocalAncestral, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A25LocalAncestralNgFactory = ComponentFactory<import1.A25LocalAncestral>('a25-local-ancestral', viewFactory_A25LocalAncestralHost0);
ComponentFactory<import1.A25LocalAncestral> get A25LocalAncestralNgFactory {
  return _A25LocalAncestralNgFactory;
}

ComponentFactory<import1.A25LocalAncestral> createA25LocalAncestralFactory() {
  return ComponentFactory('a25-local-ancestral', viewFactory_A25LocalAncestralHost0);
}

class _ViewA25LocalAncestral1 extends import14.EmbeddedView<import1.A25LocalAncestral> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewA25LocalAncestral1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_A25LocalAncestral2);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_g = import7.unsafeCast<import1.Grupo>(this.locals['\$implicit']);
    final currVal_0 = local_g.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'g.itens', 'package:corpus_ngdart/src/a25_local_ancestral.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/a25_local_ancestral.html:51:79 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import14.EmbeddedView<void> viewFactory_A25LocalAncestral1(import15.RenderView parentView, int parentIndex) {
  return _ViewA25LocalAncestral1(parentView, parentIndex);
}

class _ViewA25LocalAncestral2 extends import14.EmbeddedView<import1.A25LocalAncestral> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  final import16.TextBinding _textBinding_3 = import16.TextBinding();
  late final ViewContainer _appEl_5;
  late final NgIf _NgIf_5_9;
  _ViewA25LocalAncestral2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('span'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import9.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    final _text_4 = import9.appendText(_el_0, ' ');
    final _anchor_5 = import9.appendAnchor(_el_0);
    this._appEl_5 = ViewContainer(5, 0, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_A25LocalAncestral3);
    this._NgIf_5_9 = NgIf(this._appEl_5, _TemplateRef_5_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_5, this._NgIf_5_9);
    }
    _el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_g = import7.unsafeCast<import1.Grupo>(import7.unsafeCast<_ViewA25LocalAncestral1>((this.parentView!)).locals['\$implicit']);
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_5_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_5_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/a25_local_ancestral.html:131:146 */;
    this._appEl_5.detectChangesInNestedViews();
    this._textBinding_1.updateText(import19.interpolateString0(local_g.nome)) /* REF:package:corpus_ngdart/src/a25_local_ancestral.html:108:118 */;
    this._textBinding_3.updateText(import19.interpolateString0(local_item)) /* REF:package:corpus_ngdart/src/a25_local_ancestral.html:119:127 */;
  }

  @override
  void destroyInternal() {
    this._appEl_5.destroyNestedViews();
  }

  void _handleEvent_0($event) {
    final local_g = import7.unsafeCast<import1.Grupo>(import7.unsafeCast<_ViewA25LocalAncestral1>((this.parentView!)).locals['\$implicit']);
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    final _ctx = this.ctx;
    _ctx.escolher(local_g, local_item);
  }
}

import14.EmbeddedView<void> viewFactory_A25LocalAncestral2(import15.RenderView parentView, int parentIndex) {
  return _ViewA25LocalAncestral2(parentView, parentIndex);
}

class _ViewA25LocalAncestral3 extends import14.EmbeddedView<import1.A25LocalAncestral> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  final import16.TextBinding _textBinding_3 = import16.TextBinding();
  _ViewA25LocalAncestral3(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('b'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import9.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_i = import7.unsafeCast<int>(import7.unsafeCast<_ViewA25LocalAncestral1>(((this.parentView!).parentView!)).locals['index']);
    final local_g = import7.unsafeCast<import1.Grupo>(import7.unsafeCast<_ViewA25LocalAncestral1>(((this.parentView!).parentView!)).locals['\$implicit']);
    this._textBinding_1.updateTextWithPrimitive(local_i) /* REF:package:corpus_ngdart/src/a25_local_ancestral.html:147:152 */;
    this._textBinding_3.updateText(import19.interpolateString0(local_g.nome)) /* REF:package:corpus_ngdart/src/a25_local_ancestral.html:153:163 */;
  }
}

import14.EmbeddedView<void> viewFactory_A25LocalAncestral3(import15.RenderView parentView, int parentIndex) {
  return _ViewA25LocalAncestral3(parentView, parentIndex);
}

final List<Object> styles$A25LocalAncestralHost = const [];

class _ViewA25LocalAncestralHost0 extends import20.HostView<import1.A25LocalAncestral> {
  @override
  void build() {
    this.componentView = ViewA25LocalAncestral0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A25LocalAncestral();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.A25LocalAncestral> viewFactory_A25LocalAncestralHost0() {
  return _ViewA25LocalAncestralHost0();
}
