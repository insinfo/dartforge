// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i95_ref_sombreado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i95_ref_sombreado.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'package:ngdart/src/utilities.dart' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/src/runtime/interpolate.dart' as import15;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'dart:core';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$I95RefSombreado = const [];

class ViewI95RefSombreado0 extends import0.ComponentView<import1.I95RefSombreado> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  late final ViewContainer _appEl_3;
  late final import5.NgFor _NgFor_3_9;
  Object? _expr_0;
  late final import6.InputElement _el_0;
  static import7.ComponentStyles? _componentStyles;
  ViewI95RefSombreado0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import10.unsafeCast(import6.document.createElement('i95-ref-sombreado'));
  }
  static String? get _debugComponentUrl {
    return (import10.isDevMode ? 'asset:corpus_ngdart/lib/src/i95_ref_sombreado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    this._el_0 = import11.appendElement<import6.InputElement>(doc, parentRenderNode, 'input');
    parentRenderNode.append(this._textBinding_1.element);
    final _anchor_2 = import11.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_I95RefSombreado1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    final _anchor_3 = import11.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I95RefSombreado2);
    this._NgFor_3_9 = import5.NgFor(this._appEl_3, _TemplateRef_3_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_3, this._NgFor_3_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_a = this._el_0;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_2_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i95_ref_sombreado.html:26:41 */;
    final currVal_0 = _ctx.itens;
    if (import14.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i95_ref_sombreado.html')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_3_9, 'ngForOf', currVal_0);
      }
      this._NgFor_3_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i95_ref_sombreado.html:72:95 */;
      this._expr_0 = currVal_0;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_3_9.ngDoCheck();
    }
    this._appEl_2.detectChangesInNestedViews();
    this._appEl_3.detectChangesInNestedViews();
    this._textBinding_1.updateText(import15.interpolate0(local_a.value)) /* REF:package:corpus_ngdart/src/i95_ref_sombreado.html:10:21 */;
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$I95RefSombreado, _debugComponentUrl));
      if (import10.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I95RefSombreadoNgFactory = ComponentFactory<import1.I95RefSombreado>('i95-ref-sombreado', viewFactory_I95RefSombreadoHost0);
ComponentFactory<import1.I95RefSombreado> get I95RefSombreadoNgFactory {
  return _I95RefSombreadoNgFactory;
}

ComponentFactory<import1.I95RefSombreado> createI95RefSombreadoFactory() {
  return ComponentFactory('i95-ref-sombreado', viewFactory_I95RefSombreadoHost0);
}

class _ViewI95RefSombreado1 extends import17.EmbeddedView<import1.I95RefSombreado> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  late final import6.InputElement _el_1;
  _ViewI95RefSombreado1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import10.unsafeCast(doc.createElement('div'));
    this._el_1 = import11.appendElement<import6.InputElement>(doc, _el_0, 'input');
    _el_0.append(this._textBinding_2.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_a = this._el_1;
    this._textBinding_2.updateText(import15.interpolate0(local_a.value)) /* REF:package:corpus_ngdart/src/i95_ref_sombreado.html:52:63 */;
  }
}

import17.EmbeddedView<void> viewFactory_I95RefSombreado1(import18.RenderView parentView, int parentIndex) {
  return _ViewI95RefSombreado1(parentView, parentIndex);
}

class _ViewI95RefSombreado2 extends import17.EmbeddedView<import1.I95RefSombreado> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  _ViewI95RefSombreado2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import10.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_a = import10.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import15.interpolateString0(local_a)) /* REF:package:corpus_ngdart/src/i95_ref_sombreado.html:96:101 */;
  }
}

import17.EmbeddedView<void> viewFactory_I95RefSombreado2(import18.RenderView parentView, int parentIndex) {
  return _ViewI95RefSombreado2(parentView, parentIndex);
}

final List<Object> styles$I95RefSombreadoHost = const [];

class _ViewI95RefSombreadoHost0 extends import20.HostView<import1.I95RefSombreado> {
  @override
  void build() {
    this.componentView = ViewI95RefSombreado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I95RefSombreado();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.I95RefSombreado> viewFactory_I95RefSombreadoHost0() {
  return _ViewI95RefSombreadoHost0();
}
