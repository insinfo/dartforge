// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i54_pipes_aninhados.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i54_pipes_aninhados.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/common/pipes/lowercase_pipe.dart' as import3;
import 'dart:core';
import 'package:ngdart/src/common/pipes/date_pipe.dart' as import5;
import 'package:ngdart/src/common/pipes/uppercase_pipe.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import8;
import 'package:ngdart/src/core/linker/views/view.dart' as import9;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import10;
import 'package:ngdart/src/utilities.dart' as import11;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import12;
import 'package:ngdart/src/runtime/proxies.dart' as import13;
import 'package:ngdart/src/runtime/interpolate.dart' as import14;
import 'package:ngdart/src/runtime/check_binding.dart' as import15;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$I54PipesAninhados = const [];

class ViewI54PipesAninhados0 extends import0.ComponentView<import1.I54PipesAninhados> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  Object? _expr_0;
  late final import3.LowerCasePipe _pipe_lowercase_0;
  late final String? Function(String?) _pipe_lowercase_0_0;
  late final String? Function(String?) _pipe_lowercase_0_1;
  late final String? Function(String?) _pipe_lowercase_0_2;
  late final String? Function(String?) _pipe_lowercase_0_3;
  late final import5.DatePipe _pipe_date_1;
  late final String? Function(dynamic, String) _pipe_date_1_0;
  late final import6.UpperCasePipe _pipe_uppercase_2;
  late final String? Function(String?) _pipe_uppercase_2_0;
  late final import7.HtmlElement _el_2;
  static import8.ComponentStyles? _componentStyles;
  ViewI54PipesAninhados0(import9.View parentView, int parentIndex) : super(parentView, parentIndex, import10.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import11.unsafeCast(import7.document.createElement('i54-pipes-aninhados'));
  }
  static String? get _debugComponentUrl {
    return (import11.isDevMode ? 'asset:corpus_ngdart/lib/src/i54_pipes_aninhados.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import12.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    this._el_2 = import12.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_2.append(this._textBinding_3.element);
    this._pipe_lowercase_0 = import3.LowerCasePipe();
    this._pipe_lowercase_0_0 = import13.pureProxy1(this._pipe_lowercase_0.transform);
    this._pipe_lowercase_0_1 = import13.pureProxy1(this._pipe_lowercase_0.transform);
    this._pipe_lowercase_0_2 = import13.pureProxy1(this._pipe_lowercase_0.transform);
    this._pipe_lowercase_0_3 = import13.pureProxy1(this._pipe_lowercase_0.transform);
    this._pipe_date_1 = import5.DatePipe();
    this._pipe_date_1_0 = import13.pureProxy2(this._pipe_date_1.transform);
    this._pipe_uppercase_2 = import6.UpperCasePipe();
    this._pipe_uppercase_2_0 = import13.pureProxy1(this._pipe_uppercase_2.transform);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import14.interpolate0(this._pipe_date_1_0(this._pipe_lowercase_0_0(_ctx.nome), _ctx.fmt))) /* REF:package:corpus_ngdart/src/i54_pipes_aninhados.html:3:47 */;
    final currVal_0 = this._pipe_uppercase_2_0(this._pipe_lowercase_0_1(_ctx.nome));
    if (import15.checkBinding(this._expr_0, currVal_0, '\$pipe.uppercase(\$pipe.lowercase(nome))', 'package:corpus_ngdart/src/i54_pipes_aninhados.html')) {
      import12.setProperty(this._el_2, 'title', currVal_0) /* REF:package:corpus_ngdart/src/i54_pipes_aninhados.html:54:102 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_3.updateText(import14.interpolate0(this._pipe_lowercase_0_3(this._pipe_lowercase_0_2(_ctx.nome)))) /* REF:package:corpus_ngdart/src/i54_pipes_aninhados.html:103:147 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import8.ComponentStyles.unscoped(styles$I54PipesAninhados, _debugComponentUrl));
      if (import11.isDevMode) {
        import8.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I54PipesAninhadosNgFactory = ComponentFactory<import1.I54PipesAninhados>('i54-pipes-aninhados', viewFactory_I54PipesAninhadosHost0);
ComponentFactory<import1.I54PipesAninhados> get I54PipesAninhadosNgFactory {
  return _I54PipesAninhadosNgFactory;
}

ComponentFactory<import1.I54PipesAninhados> createI54PipesAninhadosFactory() {
  return ComponentFactory('i54-pipes-aninhados', viewFactory_I54PipesAninhadosHost0);
}

final List<Object> styles$I54PipesAninhadosHost = const [];

class _ViewI54PipesAninhadosHost0 extends import17.HostView<import1.I54PipesAninhados> {
  @override
  void build() {
    this.componentView = ViewI54PipesAninhados0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I54PipesAninhados();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.I54PipesAninhados> viewFactory_I54PipesAninhadosHost0() {
  return _ViewI54PipesAninhadosHost0();
}
