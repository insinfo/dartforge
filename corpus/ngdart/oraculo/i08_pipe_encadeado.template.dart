// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i08_pipe_encadeado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i08_pipe_encadeado.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/common/pipes/lowercase_pipe.dart' as import3;
import 'dart:core';
import 'package:ngdart/src/common/pipes/uppercase_pipe.dart' as import5;
import 'package:ngdart/src/common/pipes/date_pipe.dart' as import6;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'package:ngdart/src/utilities.dart' as import10;
import 'dart:html' as import11;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import12;
import 'package:ngdart/src/runtime/proxies.dart' as import13;
import 'package:ngdart/src/runtime/interpolate.dart' as import14;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$I08PipeEncadeado = const [];

class ViewI08PipeEncadeado0 extends import0.ComponentView<import1.I08PipeEncadeado> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  late final import3.LowerCasePipe _pipe_lowercase_0;
  late final String? Function(String?) _pipe_lowercase_0_0;
  late final import5.UpperCasePipe _pipe_uppercase_1;
  late final String? Function(String?) _pipe_uppercase_1_0;
  late final import6.DatePipe _pipe_date_2;
  late final String? Function(dynamic, String) _pipe_date_2_0;
  static import7.ComponentStyles? _componentStyles;
  ViewI08PipeEncadeado0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import10.unsafeCast(import11.document.createElement('i08-pipe-encadeado'));
  }
  static String? get _debugComponentUrl {
    return (import10.isDevMode ? 'asset:corpus_ngdart/lib/src/i08_pipe_encadeado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import11.document;
    final _el_0 = import12.appendElement<import11.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    final _el_2 = import12.appendElement<import11.HtmlElement>(doc, parentRenderNode, 'p');
    _el_2.append(this._textBinding_3.element);
    this._pipe_lowercase_0 = import3.LowerCasePipe();
    this._pipe_lowercase_0_0 = import13.pureProxy1(this._pipe_lowercase_0.transform);
    this._pipe_uppercase_1 = import5.UpperCasePipe();
    this._pipe_uppercase_1_0 = import13.pureProxy1(this._pipe_uppercase_1.transform);
    this._pipe_date_2 = import6.DatePipe();
    this._pipe_date_2_0 = import13.pureProxy2(this._pipe_date_2.transform);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import14.interpolate0(this._pipe_uppercase_1_0(this._pipe_lowercase_0_0(_ctx.nome)))) /* REF:package:corpus_ngdart/src/i08_pipe_encadeado.html:3:47 */;
    this._textBinding_3.updateText(import14.interpolate0(this._pipe_date_2_0(_ctx.quando, 'dd/MM'))) /* REF:package:corpus_ngdart/src/i08_pipe_encadeado.html:54:87 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$I08PipeEncadeado, _debugComponentUrl));
      if (import10.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I08PipeEncadeadoNgFactory = ComponentFactory<import1.I08PipeEncadeado>('i08-pipe-encadeado', viewFactory_I08PipeEncadeadoHost0);
ComponentFactory<import1.I08PipeEncadeado> get I08PipeEncadeadoNgFactory {
  return _I08PipeEncadeadoNgFactory;
}

ComponentFactory<import1.I08PipeEncadeado> createI08PipeEncadeadoFactory() {
  return ComponentFactory('i08-pipe-encadeado', viewFactory_I08PipeEncadeadoHost0);
}

final List<Object> styles$I08PipeEncadeadoHost = const [];

class _ViewI08PipeEncadeadoHost0 extends import16.HostView<import1.I08PipeEncadeado> {
  @override
  void build() {
    this.componentView = ViewI08PipeEncadeado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I08PipeEncadeado();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.I08PipeEncadeado> viewFactory_I08PipeEncadeadoHost0() {
  return _ViewI08PipeEncadeadoHost0();
}
