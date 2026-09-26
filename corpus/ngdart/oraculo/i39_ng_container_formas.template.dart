// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i39_ng_container_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i39_ng_container_formas.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$I39NgContainerFormas = const [];

class ViewI39NgContainerFormas0 extends import0.ComponentView<import1.I39NgContainerFormas> {
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  late final ViewContainer _appEl_5;
  late final import5.NgFor _NgFor_5_9;
  Object? _expr_0;
  static import6.ComponentStyles? _componentStyles;
  ViewI39NgContainerFormas0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('i39-ng-container-formas'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/i39_ng_container_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import10.document;
    final _el_0 = import11.appendDiv(doc, parentRenderNode);
    final _text_1 = import11.appendText(_el_0, 'a');
    final _el_2 = import11.appendElement<import10.HtmlElement>(doc, _el_0, 'b');
    _el_2.append(this._textBinding_3.element);
    final _anchor_4 = import11.appendAnchor(parentRenderNode);
    this._appEl_4 = ViewContainer(4, null, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_I39NgContainerFormas1);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    final _anchor_5 = import11.appendAnchor(parentRenderNode);
    this._appEl_5 = ViewContainer(5, null, this, _anchor_5);
    var _TemplateRef_5_8 = TemplateRef(this._appEl_5, viewFactory_I39NgContainerFormas3);
    this._NgFor_5_9 = import5.NgFor(this._appEl_5, _TemplateRef_5_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_5, this._NgFor_5_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_4_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i39_ng_container_formas.html:67:82 */;
    final currVal_0 = _ctx.itens;
    if (import14.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i39_ng_container_formas.html')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_5_9, 'ngForOf', currVal_0);
      }
      this._NgFor_5_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i39_ng_container_formas.html:142:165 */;
      this._expr_0 = currVal_0;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_5_9.ngDoCheck();
    }
    this._appEl_4.detectChangesInNestedViews();
    this._appEl_5.detectChangesInNestedViews();
    this._textBinding_3.updateTextWithPrimitive(_ctx.n) /* REF:package:corpus_ngdart/src/i39_ng_container_formas.html:23:28 */;
  }

  @override
  void destroyInternal() {
    this._appEl_4.destroyNestedViews();
    this._appEl_5.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$I39NgContainerFormas, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I39NgContainerFormasNgFactory = ComponentFactory<import1.I39NgContainerFormas>('i39-ng-container-formas', viewFactory_I39NgContainerFormasHost0);
ComponentFactory<import1.I39NgContainerFormas> get I39NgContainerFormasNgFactory {
  return _I39NgContainerFormasNgFactory;
}

ComponentFactory<import1.I39NgContainerFormas> createI39NgContainerFormasFactory() {
  return ComponentFactory('i39-ng-container-formas', viewFactory_I39NgContainerFormasHost0);
}

class _ViewI39NgContainerFormas1 extends import16.EmbeddedView<import1.I39NgContainerFormas> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  _ViewI39NgContainerFormas1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _anchor_0 = import11.createAnchor();
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I39NgContainerFormas2);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final doc = import10.document;
    final _el_1 = import9.unsafeCast(doc.createElement('i'));
    final _text_2 = import11.appendText(_el_1, 'y');
    this.initRootNodesAndSubscriptions(import9.unsafeCast(<Object>[this._appEl_0, _el_1]), null);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', (_ctx.n > 0));
    }
    this._NgIf_0_9.ngIf = (_ctx.n > 0) /* REF:package:corpus_ngdart/src/i39_ng_container_formas.html:86:99 */;
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }
}

import16.EmbeddedView<void> viewFactory_I39NgContainerFormas1(import17.RenderView parentView, int parentIndex) {
  return _ViewI39NgContainerFormas1(parentView, parentIndex);
}

class _ViewI39NgContainerFormas2 extends import16.EmbeddedView<import1.I39NgContainerFormas> {
  _ViewI39NgContainerFormas2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('p'));
    final _text_1 = import11.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import16.EmbeddedView<void> viewFactory_I39NgContainerFormas2(import17.RenderView parentView, int parentIndex) {
  return _ViewI39NgContainerFormas2(parentView, parentIndex);
}

class _ViewI39NgContainerFormas3 extends import16.EmbeddedView<import1.I39NgContainerFormas> {
  final import2.TextBinding _textBinding_0 = import2.TextBinding();
  _ViewI39NgContainerFormas3(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_1 = import9.unsafeCast(doc.createElement('b'));
    final _text_2 = import11.appendText(_el_1, '.');
    _el_1.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    this.initRootNodesAndSubscriptions(import9.unsafeCast(<Object>[this._textBinding_0.element, _el_1]), null);
  }

  @override
  void detectChangesInternal() {
    final local_x = import9.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_0.updateText(import19.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/i39_ng_container_formas.html:166:171 */;
  }

  void _handleEvent_0($event) {
    final local_x = import9.unsafeCast<String>(this.locals['\$implicit']);
    final _ctx = this.ctx;
    _ctx.usar(local_x);
  }
}

import16.EmbeddedView<void> viewFactory_I39NgContainerFormas3(import17.RenderView parentView, int parentIndex) {
  return _ViewI39NgContainerFormas3(parentView, parentIndex);
}

final List<Object> styles$I39NgContainerFormasHost = const [];

class _ViewI39NgContainerFormasHost0 extends import20.HostView<import1.I39NgContainerFormas> {
  @override
  void build() {
    this.componentView = ViewI39NgContainerFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I39NgContainerFormas();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.I39NgContainerFormas> viewFactory_I39NgContainerFormasHost0() {
  return _ViewI39NgContainerFormasHost0();
}
