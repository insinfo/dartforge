// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c17_pipe_com_argumento.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c17_pipe_com_argumento.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/common/pipes/date_pipe.dart' as import5;
import 'dart:core';
import 'package:ngdart/src/common/pipes/uppercase_pipe.dart' as import7;
import 'package:ngdart/src/common/pipes/lowercase_pipe.dart' as import8;
import 'e04_pipe.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import11;
import 'package:ngdart/src/core/linker/views/view.dart' as import12;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import13;
import 'package:ngdart/src/utilities.dart' as import14;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import15;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import17;
import 'package:ngdart/src/runtime/proxies.dart' as import18;
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/src/runtime/check_binding.dart' as import20;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import22;
import 'package:ngdart/src/common/directives/ng_for.dart' as import23;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import24;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import25;

final List<Object> styles$C17PipeComArgumento = const [];

class ViewC17PipeComArgumento0 extends import0.ComponentView<import1.C17PipeComArgumento> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_4 = import2.TextBinding();
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  Object? _expr_0;
  late final import5.DatePipe _pipe_date_0;
  late final String? Function(dynamic, String) _pipe_date_0_0;
  late final String? Function(dynamic) _pipe_date_0_1;
  late final import7.UpperCasePipe _pipe_uppercase_1;
  late final String? Function(String?) _pipe_uppercase_1_0;
  late final import8.LowerCasePipe _pipe_lowercase_2;
  late final import9.E04Pipe _pipe_e04_3;
  late final import10.HtmlElement _el_0;
  static import11.ComponentStyles? _componentStyles;
  ViewC17PipeComArgumento0(import12.View parentView, int parentIndex) : super(parentView, parentIndex, import13.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import14.unsafeCast(import10.document.createElement('c17-pipe-com-argumento'));
  }
  static String? get _debugComponentUrl {
    return (import14.isDevMode ? 'asset:corpus_ngdart/lib/src/c17_pipe_com_argumento.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import10.document;
    this._el_0 = import15.appendElement<import10.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_0.append(this._textBinding_1.element);
    final _anchor_2 = import15.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_C17PipeComArgumento1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import17.isDevToolsEnabled) {
      import17.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    final _el_3 = import15.appendSpan(doc, parentRenderNode);
    _el_3.append(this._textBinding_4.element);
    this._pipe_date_0 = import5.DatePipe();
    this._pipe_date_0_0 = import18.pureProxy2(this._pipe_date_0.transform);
    this._pipe_date_0_1 = import18.pureProxy1(this._pipe_date_0.transform);
    this._pipe_uppercase_1 = import7.UpperCasePipe();
    this._pipe_uppercase_1_0 = import18.pureProxy1(this._pipe_uppercase_1.transform);
    this._pipe_lowercase_2 = import8.LowerCasePipe();
    this._pipe_e04_3 = import9.E04Pipe();
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import17.isDevToolsEnabled) {
      import17.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostra);
    }
    this._NgIf_2_9.ngIf = _ctx.mostra /* REF:package:corpus_ngdart/src/c17_pipe_com_argumento.html:86:100 */;
    this._appEl_2.detectChangesInNestedViews();
    final currVal_0 = import19.interpolate0(this._pipe_date_0_0(_ctx.quando, 'dd/MM/yyyy'));
    if (import20.checkBinding(this._expr_0, currVal_0, '{{ \$pipe.date(quando, \'dd/MM/yyyy\') }}', 'package:corpus_ngdart/src/c17_pipe_com_argumento.html')) {
      import15.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/c17_pipe_com_argumento.html:3:49 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import19.interpolate0(this._pipe_uppercase_1_0(_ctx.nome))) /* REF:package:corpus_ngdart/src/c17_pipe_com_argumento.html:50:77 */;
    this._textBinding_4.updateText(import19.interpolate0(this._pipe_date_0_1(_ctx.quando))) /* REF:package:corpus_ngdart/src/c17_pipe_com_argumento.html:230:254 */;
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
      _componentStyles = (styles = import11.ComponentStyles.unscoped(styles$C17PipeComArgumento, _debugComponentUrl));
      if (import14.isDevMode) {
        import11.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C17PipeComArgumentoNgFactory = ComponentFactory<import1.C17PipeComArgumento>('c17-pipe-com-argumento', viewFactory_C17PipeComArgumentoHost0);
ComponentFactory<import1.C17PipeComArgumento> get C17PipeComArgumentoNgFactory {
  return _C17PipeComArgumentoNgFactory;
}

ComponentFactory<import1.C17PipeComArgumento> createC17PipeComArgumentoFactory() {
  return ComponentFactory('c17-pipe-com-argumento', viewFactory_C17PipeComArgumentoHost0);
}

class _ViewC17PipeComArgumento1 extends import22.EmbeddedView<import1.C17PipeComArgumento> {
  late final ViewContainer _appEl_1;
  late final import23.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewC17PipeComArgumento1(import24.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import14.unsafeCast(doc.createElement('ul'));
    final _anchor_1 = import15.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_C17PipeComArgumento2);
    this._NgFor_1_9 = import23.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import17.isDevToolsEnabled) {
      import17.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import20.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/c17_pipe_com_argumento.html')) {
      if (import17.isDevToolsEnabled) {
        import17.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/c17_pipe_com_argumento.html:108:134 */;
      this._expr_0 = currVal_0;
    }
    if ((!import20.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import22.EmbeddedView<void> viewFactory_C17PipeComArgumento1(import24.RenderView parentView, int parentIndex) {
  return _ViewC17PipeComArgumento1(parentView, parentIndex);
}

class _ViewC17PipeComArgumento2 extends import22.EmbeddedView<import1.C17PipeComArgumento> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  final import2.TextBinding _textBinding_5 = import2.TextBinding();
  late final String? Function(String?) _pipe_uppercase_1_1;
  late final String? Function(String?) _pipe_lowercase_2_0;
  late final String Function(String) _pipe_e04_3_0;
  _ViewC17PipeComArgumento2(import24.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import14.unsafeCast(doc.createElement('li'));
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import15.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    final _text_4 = import15.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_5.element);
    this._pipe_uppercase_1_1 = import18.pureProxy1(import14.unsafeCast<ViewC17PipeComArgumento0>(((this.parentView!).parentView!))._pipe_uppercase_1.transform);
    this._pipe_lowercase_2_0 = import18.pureProxy1(import14.unsafeCast<ViewC17PipeComArgumento0>(((this.parentView!).parentView!))._pipe_lowercase_2.transform);
    this._pipe_e04_3_0 = import18.pureProxy1(import14.unsafeCast<ViewC17PipeComArgumento0>(((this.parentView!).parentView!))._pipe_e04_3.transform);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_item = import14.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import19.interpolate0(this._pipe_uppercase_1_1(local_item))) /* REF:package:corpus_ngdart/src/c17_pipe_com_argumento.html:135:162 */;
    this._textBinding_3.updateText(import19.interpolate0(this._pipe_lowercase_2_0(local_item))) /* REF:package:corpus_ngdart/src/c17_pipe_com_argumento.html:163:190 */;
    this._textBinding_5.updateText(import19.interpolate0(this._pipe_e04_3_0(local_item))) /* REF:package:corpus_ngdart/src/c17_pipe_com_argumento.html:191:212 */;
  }
}

import22.EmbeddedView<void> viewFactory_C17PipeComArgumento2(import24.RenderView parentView, int parentIndex) {
  return _ViewC17PipeComArgumento2(parentView, parentIndex);
}

final List<Object> styles$C17PipeComArgumentoHost = const [];

class _ViewC17PipeComArgumentoHost0 extends import25.HostView<import1.C17PipeComArgumento> {
  @override
  void build() {
    this.componentView = ViewC17PipeComArgumento0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C17PipeComArgumento();
    this.initRootNode(_el_0);
  }
}

import25.HostView<import1.C17PipeComArgumento> viewFactory_C17PipeComArgumentoHost0() {
  return _ViewC17PipeComArgumentoHost0();
}
