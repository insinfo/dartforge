// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i37_usa_callback.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i37_usa_callback.dart' as import1;
import 'i37_filho_callback.template.dart' as import2;
import 'i37_filho_callback.dart' as import3;
import 'package:ngdart/src/core/linker/view_container.dart';
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
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'dart:core';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$I37UsaCallback = const [];

class ViewI37UsaCallback0 extends import0.ComponentView<import1.I37UsaCallback> {
  late final import2.ViewI37FilhoCallback0 _compView_0;
  late final import3.I37FilhoCallback _I37FilhoCallback_0_5;
  late final ViewContainer _appEl_2;
  late final import5.NgFor _NgFor_2_9;
  Object? _expr_1;
  static import6.ComponentStyles? _componentStyles;
  ViewI37UsaCallback0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('i37-usa-callback'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/i37_usa_callback.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewI37FilhoCallback0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I37FilhoCallback_0_5 = import3.I37FilhoCallback();
    this._compView_0.create(this._I37FilhoCallback_0_5);
    final doc = import10.document;
    final _el_1 = import11.appendElement<import10.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_2 = import11.appendAnchor(_el_1);
    this._appEl_2 = ViewContainer(2, 1, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_I37UsaCallback1);
    this._NgFor_2_9 = import5.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if ((_ctx.salvar != null)) {
        if (import13.isDevToolsEnabled) {
          import13.Inspector.instance.recordInput(this._I37FilhoCallback_0_5, 'acao', _ctx.salvar);
        }
        this._I37FilhoCallback_0_5.acao = _ctx.salvar /* REF:package:corpus_ngdart/src/i37_usa_callback.html:20:35 */;
      }
      if ((_ctx.rastrear != null)) {
        if (import13.isDevToolsEnabled) {
          import13.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForTrackBy', _ctx.rastrear);
        }
        this._NgFor_2_9.ngForTrackBy = _ctx.rastrear /* REF:package:corpus_ngdart/src/i37_usa_callback.html:65:122 */;
      }
    }
    final currVal_1 = _ctx.itens;
    if (import14.checkBinding(this._expr_1, currVal_1, 'itens', 'package:corpus_ngdart/src/i37_usa_callback.html')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_1);
      }
      this._NgFor_2_9.ngForOf = currVal_1 /* REF:package:corpus_ngdart/src/i37_usa_callback.html:65:122 */;
      this._expr_1 = currVal_1;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_2_9.ngDoCheck();
    }
    this._appEl_2.detectChangesInNestedViews();
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$I37UsaCallback, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I37UsaCallbackNgFactory = ComponentFactory<import1.I37UsaCallback>('i37-usa-callback', viewFactory_I37UsaCallbackHost0);
ComponentFactory<import1.I37UsaCallback> get I37UsaCallbackNgFactory {
  return _I37UsaCallbackNgFactory;
}

ComponentFactory<import1.I37UsaCallback> createI37UsaCallbackFactory() {
  return ComponentFactory('i37-usa-callback', viewFactory_I37UsaCallbackHost0);
}

class _ViewI37UsaCallback1 extends import16.EmbeddedView<import1.I37UsaCallback> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewI37UsaCallback1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('li'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_i = import9.unsafeCast<int>(this.locals['index']);
    this._textBinding_1.updateTextWithPrimitive(local_i) /* REF:package:corpus_ngdart/src/i37_usa_callback.html:123:128 */;
  }
}

import16.EmbeddedView<void> viewFactory_I37UsaCallback1(import18.RenderView parentView, int parentIndex) {
  return _ViewI37UsaCallback1(parentView, parentIndex);
}

final List<Object> styles$I37UsaCallbackHost = const [];

class _ViewI37UsaCallbackHost0 extends import20.HostView<import1.I37UsaCallback> {
  @override
  void build() {
    this.componentView = ViewI37UsaCallback0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I37UsaCallback();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.I37UsaCallback> viewFactory_I37UsaCallbackHost0() {
  return _ViewI37UsaCallbackHost0();
}
