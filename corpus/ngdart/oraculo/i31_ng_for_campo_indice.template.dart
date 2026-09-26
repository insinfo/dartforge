// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i31_ng_for_campo_indice.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i31_ng_for_campo_indice.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/src/runtime/interpolate.dart' as import14;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import18;

final List<Object> styles$I31NgForCampoIndice = const [];

class ViewI31NgForCampoIndice0 extends import0.ComponentView<import1.I31NgForCampoIndice> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  late final ViewContainer _appEl_0;
  late final import4.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewI31NgForCampoIndice0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('i31-ng-for-campo-indice'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/i31_ng_for_campo_indice.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import10.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I31NgForCampoIndice1);
    this._NgFor_0_9 = import4.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
    final doc = import9.document;
    final _el_1 = import10.appendElement<import9.HtmlElement>(doc, parentRenderNode, 'p');
    _el_1.append(this._textBinding_2.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.matriz[0];
    if (import13.checkBinding(this._expr_0, currVal_0, 'matriz[0]', 'package:corpus_ngdart/src/i31_ng_for_campo_indice.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i31_ng_for_campo_indice.html:3:30 */;
      this._expr_0 = currVal_0;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
    this._textBinding_2.updateText(import14.interpolate0(_ctx.mapa['a'])) /* REF:package:corpus_ngdart/src/i31_ng_for_campo_indice.html:43:56 */;
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
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$I31NgForCampoIndice, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I31NgForCampoIndiceNgFactory = ComponentFactory<import1.I31NgForCampoIndice>('i31-ng-for-campo-indice', viewFactory_I31NgForCampoIndiceHost0);
ComponentFactory<import1.I31NgForCampoIndice> get I31NgForCampoIndiceNgFactory {
  return _I31NgForCampoIndiceNgFactory;
}

ComponentFactory<import1.I31NgForCampoIndice> createI31NgForCampoIndiceFactory() {
  return ComponentFactory('i31-ng-for-campo-indice', viewFactory_I31NgForCampoIndiceHost0);
}

class _ViewI31NgForCampoIndice1 extends import16.EmbeddedView<import1.I31NgForCampoIndice> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  _ViewI31NgForCampoIndice1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_x = this.locals['\$implicit'];
    this._textBinding_1.updateText(import14.interpolate0(local_x)) /* REF:package:corpus_ngdart/src/i31_ng_for_campo_indice.html:31:36 */;
  }
}

import16.EmbeddedView<void> viewFactory_I31NgForCampoIndice1(import17.RenderView parentView, int parentIndex) {
  return _ViewI31NgForCampoIndice1(parentView, parentIndex);
}

final List<Object> styles$I31NgForCampoIndiceHost = const [];

class _ViewI31NgForCampoIndiceHost0 extends import18.HostView<import1.I31NgForCampoIndice> {
  @override
  void build() {
    this.componentView = ViewI31NgForCampoIndice0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I31NgForCampoIndice();
    this.initRootNode(_el_0);
  }
}

import18.HostView<import1.I31NgForCampoIndice> viewFactory_I31NgForCampoIndiceHost0() {
  return _ViewI31NgForCampoIndiceHost0();
}
