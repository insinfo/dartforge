// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j122_ordem_dos_nos_promovidos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j122_ordem_dos_nos_promovidos.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'dart:html' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$J122Usa = const [];

class ViewJ122Usa0 extends import0.ComponentView<import1.J122Usa> {
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import4.HtmlElement _el_2;
  late final import4.DivElement _el_0;
  late final import4.HtmlElement _el_1;
  static import5.ComponentStyles? _componentStyles;
  ViewJ122Usa0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import4.document.createElement('j122-usa'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import4.document;
    this._el_0 = import9.appendDiv(doc, parentRenderNode);
    this._el_1 = import9.appendElement<import4.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_2 = import9.appendSpan(doc, parentRenderNode);
    final _anchor_3 = import9.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J122Usa1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_depois = this._el_2;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_3_9.ngIf = _ctx.mostra /* REF:asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart:535:549 */;
    this._appEl_3.detectChangesInNestedViews();
    final currVal_0 = _ctx.v;
    if (import12.checkBinding(this._expr_0, currVal_0, 'v', 'asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart')) {
      import9.updateAttribute(this._el_0, 'a', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart:443:455 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = local_depois.id;
    if (import12.checkBinding(this._expr_1, currVal_1, 'depois.id', 'asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart')) {
      import9.setProperty(this._el_1, 'title', currVal_1) /* REF:asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart:466:485 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.v;
    if (import12.checkBinding(this._expr_2, currVal_2, 'v', 'asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart')) {
      import9.updateAttribute(this._el_2, 'b', currVal_2) /* REF:asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart:505:517 */;
      this._expr_2 = currVal_2;
    }
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$J122Usa, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J122UsaNgFactory = ComponentFactory<import1.J122Usa>('j122-usa', viewFactory_J122UsaHost0);
ComponentFactory<import1.J122Usa> get J122UsaNgFactory {
  return _J122UsaNgFactory;
}

ComponentFactory<import1.J122Usa> createJ122UsaFactory() {
  return ComponentFactory('j122-usa', viewFactory_J122UsaHost0);
}

class _ViewJ122Usa1 extends import14.EmbeddedView<import1.J122Usa> {
  Object? _expr_0;
  Object? _expr_1;
  late final import4.HtmlElement _el_3;
  late final import4.HtmlElement _el_1;
  late final import4.HtmlElement _el_2;
  _ViewJ122Usa1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import4.document;
    final _el_0 = import8.unsafeCast(doc.createElement('section'));
    this._el_1 = import9.appendElement<import4.HtmlElement>(doc, _el_0, 'i');
    this._el_2 = import9.appendElement<import4.HtmlElement>(doc, _el_0, 'b');
    this._el_3 = import9.appendElement<import4.HtmlElement>(doc, _el_0, 'em');
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_la = this._el_3;
    final currVal_0 = _ctx.v;
    if (import12.checkBinding(this._expr_0, currVal_0, 'v', 'asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart')) {
      import9.updateAttribute(this._el_1, 'c', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart:553:565 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = local_la.id;
    if (import12.checkBinding(this._expr_1, currVal_1, 'la.id', 'asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart')) {
      import9.setProperty(this._el_2, 'title', currVal_1) /* REF:asset:corpus_ngdart/lib/src/j122_ordem_dos_nos_promovidos.dart:573:588 */;
      this._expr_1 = currVal_1;
    }
  }
}

import14.EmbeddedView<void> viewFactory_J122Usa1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ122Usa1(parentView, parentIndex);
}

final List<Object> styles$J122UsaHost = const [];

class _ViewJ122UsaHost0 extends import16.HostView<import1.J122Usa> {
  @override
  void build() {
    this.componentView = ViewJ122Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J122Usa();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.J122Usa> viewFactory_J122UsaHost0() {
  return _ViewJ122UsaHost0();
}
