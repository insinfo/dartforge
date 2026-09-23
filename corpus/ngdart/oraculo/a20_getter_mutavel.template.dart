// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a20_getter_mutavel.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a20_getter_mutavel.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import4;
import 'dart:html' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/src/runtime/interpolate.dart' as import14;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$A20GetterMutavel = const [];

class ViewA20GetterMutavel0 extends import0.ComponentView<import1.A20GetterMutavel> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  late final ViewContainer _appEl_4;
  late final import4.NgFor _NgFor_4_9;
  Object? _expr_0;
  Object? _expr_1;
  late final import5.HtmlElement _el_2;
  static import6.ComponentStyles? _componentStyles;
  ViewA20GetterMutavel0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import5.document.createElement('a20-getter-mutavel'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/a20_getter_mutavel.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import5.document;
    final _el_0 = import10.appendElement<import5.HtmlElement>(doc, parentRenderNode, 'h1');
    _el_0.append(this._textBinding_1.element);
    this._el_2 = import10.appendElement<import5.HtmlElement>(doc, parentRenderNode, 'p');
    final _el_3 = import10.appendElement<import5.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_4 = import10.appendAnchor(_el_3);
    this._appEl_4 = ViewContainer(4, 3, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_A20GetterMutavel1);
    this._NgFor_4_9 = import4.NgFor(this._appEl_4, _TemplateRef_4_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_4, this._NgFor_4_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_1 = _ctx.itens;
    if (import13.checkBinding(this._expr_1, currVal_1, 'itens', 'package:corpus_ngdart/src/a20_getter_mutavel.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_4_9, 'ngForOf', currVal_1);
      }
      this._NgFor_4_9.ngForOf = currVal_1 /* REF:package:corpus_ngdart/src/a20_getter_mutavel.html:51:77 */;
      this._expr_1 = currVal_1;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_4_9.ngDoCheck();
    }
    this._appEl_4.detectChangesInNestedViews();
    this._textBinding_1.updateText(import14.interpolateString0(_ctx.titulo)) /* REF:package:corpus_ngdart/src/a20_getter_mutavel.html:4:14 */;
    final currVal_0 = _ctx.titulo;
    if (import13.checkBinding(this._expr_0, currVal_0, 'titulo', 'package:corpus_ngdart/src/a20_getter_mutavel.html')) {
      import10.setProperty(this._el_2, 'title', currVal_0) /* REF:package:corpus_ngdart/src/a20_getter_mutavel.html:22:38 */;
      this._expr_0 = currVal_0;
    }
  }

  @override
  void destroyInternal() {
    this._appEl_4.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$A20GetterMutavel, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A20GetterMutavelNgFactory = ComponentFactory<import1.A20GetterMutavel>('a20-getter-mutavel', viewFactory_A20GetterMutavelHost0);
ComponentFactory<import1.A20GetterMutavel> get A20GetterMutavelNgFactory {
  return _A20GetterMutavelNgFactory;
}

ComponentFactory<import1.A20GetterMutavel> createA20GetterMutavelFactory() {
  return ComponentFactory('a20-getter-mutavel', viewFactory_A20GetterMutavelHost0);
}

class _ViewA20GetterMutavel1 extends import16.EmbeddedView<import1.A20GetterMutavel> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  _ViewA20GetterMutavel1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('li'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_item = import9.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import14.interpolateString0(local_item)) /* REF:package:corpus_ngdart/src/a20_getter_mutavel.html:78:86 */;
  }
}

import16.EmbeddedView<void> viewFactory_A20GetterMutavel1(import17.RenderView parentView, int parentIndex) {
  return _ViewA20GetterMutavel1(parentView, parentIndex);
}

final List<Object> styles$A20GetterMutavelHost = const [];

class _ViewA20GetterMutavelHost0 extends import19.HostView<import1.A20GetterMutavel> {
  @override
  void build() {
    this.componentView = ViewA20GetterMutavel0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A20GetterMutavel();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.A20GetterMutavel> viewFactory_A20GetterMutavelHost0() {
  return _ViewA20GetterMutavelHost0();
}
