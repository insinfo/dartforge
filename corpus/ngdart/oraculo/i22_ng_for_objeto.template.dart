// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i22_ng_for_objeto.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i22_ng_for_objeto.dart' as import1;
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
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/interpolate.dart' as import17;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import18;

final List<Object> styles$I22NgForObjeto = const [];

class ViewI22NgForObjeto0 extends import0.ComponentView<import1.I22NgForObjeto> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI22NgForObjeto0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i22-ng-for-objeto'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i22_ng_for_objeto.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I22NgForObjeto1);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.pessoas;
    if (import12.checkBinding(this._expr_0, currVal_0, 'pessoas', 'package:corpus_ngdart/src/i22_ng_for_objeto.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i22_ng_for_objeto.html:8:33 */;
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

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I22NgForObjeto, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I22NgForObjetoNgFactory = ComponentFactory<import1.I22NgForObjeto>('i22-ng-for-objeto', viewFactory_I22NgForObjetoHost0);
ComponentFactory<import1.I22NgForObjeto> get I22NgForObjetoNgFactory {
  return _I22NgForObjetoNgFactory;
}

ComponentFactory<import1.I22NgForObjeto> createI22NgForObjetoFactory() {
  return ComponentFactory('i22-ng-for-objeto', viewFactory_I22NgForObjetoHost0);
}

class _ViewI22NgForObjeto1 extends import14.EmbeddedView<import1.I22NgForObjeto> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  final import15.TextBinding _textBinding_3 = import15.TextBinding();
  _ViewI22NgForObjeto1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('li'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import9.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    _el_0.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_p = import7.unsafeCast<import1.Pessoa>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import17.interpolateString0(local_p.nome)) /* REF:package:corpus_ngdart/src/i22_ng_for_objeto.html:56:66 */;
    this._textBinding_3.updateTextWithPrimitive(local_p.idade) /* REF:package:corpus_ngdart/src/i22_ng_for_objeto.html:67:78 */;
  }

  void _handleEvent_0($event) {
    final local_p = import7.unsafeCast<import1.Pessoa>(this.locals['\$implicit']);
    final _ctx = this.ctx;
    _ctx.escolher(local_p);
  }
}

import14.EmbeddedView<void> viewFactory_I22NgForObjeto1(import16.RenderView parentView, int parentIndex) {
  return _ViewI22NgForObjeto1(parentView, parentIndex);
}

final List<Object> styles$I22NgForObjetoHost = const [];

class _ViewI22NgForObjetoHost0 extends import18.HostView<import1.I22NgForObjeto> {
  @override
  void build() {
    this.componentView = ViewI22NgForObjeto0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I22NgForObjeto();
    this.initRootNode(_el_0);
  }
}

import18.HostView<import1.I22NgForObjeto> viewFactory_I22NgForObjetoHost0() {
  return _ViewI22NgForObjetoHost0();
}
