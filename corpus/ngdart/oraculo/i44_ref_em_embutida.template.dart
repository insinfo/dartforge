// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i44_ref_em_embutida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i44_ref_em_embutida.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
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
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/interpolate.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$I44RefEmEmbutida = const [];

class ViewI44RefEmEmbutida0 extends import0.ComponentView<import1.I44RefEmEmbutida> {
  late final ViewContainer _appEl_2;
  late final import3.NgFor _NgFor_2_9;
  Object? _expr_0;
  late final import4.InputElement _el_0;
  static import5.ComponentStyles? _componentStyles;
  ViewI44RefEmEmbutida0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import4.document.createElement('i44-ref-em-embutida'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/i44_ref_em_embutida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import4.document;
    this._el_0 = import9.appendElement<import4.InputElement>(doc, parentRenderNode, 'input');
    final _el_1 = import9.appendElement<import4.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_2 = import9.appendAnchor(_el_1);
    this._appEl_2 = ViewContainer(2, 1, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_I44RefEmEmbutida1);
    this._NgFor_2_9 = import3.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i44_ref_em_embutida.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_0);
      }
      this._NgFor_2_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i44_ref_em_embutida.html:22:45 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_2_9.ngDoCheck();
    }
    this._appEl_2.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$I44RefEmEmbutida, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I44RefEmEmbutidaNgFactory = ComponentFactory<import1.I44RefEmEmbutida>('i44-ref-em-embutida', viewFactory_I44RefEmEmbutidaHost0);
ComponentFactory<import1.I44RefEmEmbutida> get I44RefEmEmbutidaNgFactory {
  return _I44RefEmEmbutidaNgFactory;
}

ComponentFactory<import1.I44RefEmEmbutida> createI44RefEmEmbutidaFactory() {
  return ComponentFactory('i44-ref-em-embutida', viewFactory_I44RefEmEmbutidaHost0);
}

class _ViewI44RefEmEmbutida1 extends import14.EmbeddedView<import1.I44RefEmEmbutida> {
  final import15.TextBinding _textBinding_4 = import15.TextBinding();
  late final import4.InputElement _el_1;
  _ViewI44RefEmEmbutida1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import4.document;
    final _el_0 = import8.unsafeCast(doc.createElement('li'));
    this._el_1 = import9.appendElement<import4.InputElement>(doc, _el_0, 'input');
    final _el_2 = import9.appendElement<import4.ButtonElement>(doc, _el_0, 'button');
    final _text_3 = import9.appendText(_el_2, '+');
    _el_0.append(this._textBinding_4.element);
    _el_2.addEventListener('click', this.eventHandler1(this._handleEvent_0));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_campo = this._el_1;
    this._textBinding_4.updateText(import17.interpolate0(local_campo.value)) /* REF:package:corpus_ngdart/src/i44_ref_em_embutida.html:122:137 */;
  }

  void _handleEvent_0($event) {
    final local_campo = this._el_1;
    final local_busca = import8.unsafeCast<ViewI44RefEmEmbutida0>((this.parentView!))._el_0;
    final local_x = import8.unsafeCast<String>(this.locals['\$implicit']);
    final _ctx = this.ctx;
    _ctx.usar(local_campo.value, local_busca.value, local_x);
  }
}

import14.EmbeddedView<void> viewFactory_I44RefEmEmbutida1(import16.RenderView parentView, int parentIndex) {
  return _ViewI44RefEmEmbutida1(parentView, parentIndex);
}

final List<Object> styles$I44RefEmEmbutidaHost = const [];

class _ViewI44RefEmEmbutidaHost0 extends import19.HostView<import1.I44RefEmEmbutida> {
  @override
  void build() {
    this.componentView = ViewI44RefEmEmbutida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I44RefEmEmbutida();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.I44RefEmEmbutida> viewFactory_I44RefEmEmbutidaHost0() {
  return _ViewI44RefEmEmbutidaHost0();
}
