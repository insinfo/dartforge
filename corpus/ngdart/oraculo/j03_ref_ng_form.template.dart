// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j03_ref_ng_form.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j03_ref_ng_form.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngforms/src/directives/ng_form.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngforms/src/directives/control_container.dart' as import11;
import 'package:ngdart/src/runtime/interpolate.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import14;

final List<Object> styles$J03RefNgForm = const [];

class ViewJ03RefNgForm0 extends import0.ComponentView<import1.J03RefNgForm> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  late final import3.NgForm _NgForm_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewJ03RefNgForm0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j03-ref-ng-form'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j03_ref_ng_form.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.FormElement>(doc, parentRenderNode, 'form');
    this._NgForm_0_5 = import3.NgForm(null, this);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_0, this._NgForm_0_5);
    }
    final _el_1 = import9.appendElement<import8.HtmlElement>(doc, parentRenderNode, 'p');
    _el_1.append(this._textBinding_2.element);
    _el_0.addEventListener('submit', this.eventHandler1(this._NgForm_0_5.onSubmit));
    _el_0.addEventListener('reset', this.eventHandler1(this._NgForm_0_5.onReset));
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if (((identical(token, import3.NgForm) || identical(token, import11.ControlContainer)) && (0 == nodeIndex))) {
      return this._NgForm_0_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final local_f = this._NgForm_0_5;
    this._textBinding_2.updateText(import12.interpolate0(local_f.valid)) /* REF:package:corpus_ngdart/src/j03_ref_ng_form.html:28:39 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J03RefNgForm, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J03RefNgFormNgFactory = ComponentFactory<import1.J03RefNgForm>('j03-ref-ng-form', viewFactory_J03RefNgFormHost0);
ComponentFactory<import1.J03RefNgForm> get J03RefNgFormNgFactory {
  return _J03RefNgFormNgFactory;
}

ComponentFactory<import1.J03RefNgForm> createJ03RefNgFormFactory() {
  return ComponentFactory('j03-ref-ng-form', viewFactory_J03RefNgFormHost0);
}

final List<Object> styles$J03RefNgFormHost = const [];

class _ViewJ03RefNgFormHost0 extends import14.HostView<import1.J03RefNgForm> {
  @override
  void build() {
    this.componentView = ViewJ03RefNgForm0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J03RefNgForm();
    this.initRootNode(_el_0);
  }
}

import14.HostView<import1.J03RefNgForm> viewFactory_J03RefNgFormHost0() {
  return _ViewJ03RefNgFormHost0();
}
