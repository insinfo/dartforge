// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i94_ref_ng_form.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i94_ref_ng_form.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngforms/src/directives/ng_form.dart' as import3;
import 'package:ngforms/src/directives/default_value_accessor.dart' as import4;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import5;
import 'package:ngforms/src/directives/ng_control_name.dart' as import6;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'package:ngdart/src/utilities.dart' as import10;
import 'dart:html' as import11;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/meta/di_tokens.dart' as import14;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import15;
import 'package:ngforms/src/directives/ng_control.dart' as import16;
import 'package:ngforms/src/directives/control_container.dart' as import17;
import 'package:ngdart/src/runtime/check_binding.dart' as import18;
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import21;

final List<Object> styles$I94RefNgForm = const [];

class ViewI94RefNgForm0 extends import0.ComponentView<import1.I94RefNgForm> {
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  late final import3.NgForm _NgForm_0_5;
  late final import4.DefaultValueAccessor _DefaultValueAccessor_1_5;
  late final List<import5.ControlValueAccessor<dynamic>> _NgValueAccessor_1_6;
  late final import6.NgControlName _NgControlName_1_7;
  Object? _expr_0;
  static import7.ComponentStyles? _componentStyles;
  ViewI94RefNgForm0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import10.unsafeCast(import11.document.createElement('i94-ref-ng-form'));
  }
  static String? get _debugComponentUrl {
    return (import10.isDevMode ? 'asset:corpus_ngdart/lib/src/i94_ref_ng_form.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import11.document;
    final _el_0 = import12.appendElement<import11.FormElement>(doc, parentRenderNode, 'form');
    this._NgForm_0_5 = import3.NgForm(null, this);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_0, this._NgForm_0_5);
    }
    final _el_1 = import12.appendElement<import11.InputElement>(doc, _el_0, 'input');
    import12.setAttribute(_el_1, 'ngControl', 'nome');
    this._DefaultValueAccessor_1_5 = import4.DefaultValueAccessor(_el_1);
    this._NgValueAccessor_1_6 = [this._DefaultValueAccessor_1_5];
    this._NgControlName_1_7 = import6.NgControlName(this._NgForm_0_5, null, this._NgValueAccessor_1_6);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._DefaultValueAccessor_1_5);
      import13.Inspector.instance.registerDirective(_el_1, this._NgControlName_1_7);
    }
    final _el_2 = import12.appendElement<import11.HtmlElement>(doc, parentRenderNode, 'p');
    _el_2.append(this._textBinding_3.element);
    _el_0.addEventListener('submit', this.eventHandler1(this._NgForm_0_5.onSubmit));
    _el_0.addEventListener('reset', this.eventHandler1(this._NgForm_0_5.onReset));
    _el_1.addEventListener('blur', this.eventHandler0(this._DefaultValueAccessor_1_5.touchHandler));
    _el_1.addEventListener('input', this.eventHandler1(this._handleEvent_0));
    final subscription_0 = this._NgControlName_1_7.update.listen(this.eventHandler1(this._handleEvent_1));
    this.initSubscriptions([subscription_0]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 1)) {
      if ((1 == nodeIndex)) {
        if (identical(token, const import14.MultiToken<import15.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
          return this._NgValueAccessor_1_6;
        }
        if (identical(token, import16.NgControl)) {
          return this._NgControlName_1_7;
        }
      }
      if ((identical(token, import3.NgForm) || identical(token, import17.ControlContainer))) {
        return this._NgForm_0_5;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    bool firstCheck = this.firstCheck;
    final local_f = this._NgForm_0_5;
    changed = false;
    if (firstCheck) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgControlName_1_7, 'ngControl', 'nome');
      }
      this._NgControlName_1_7.name = 'nome' /* REF:package:corpus_ngdart/src/i94_ref_ng_form.html:44:60 */;
      changed = true;
    }
    final currVal_0 = _ctx.nome;
    if (import18.checkBinding(this._expr_0, currVal_0, 'nome', 'package:corpus_ngdart/src/i94_ref_ng_form.html')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgControlName_1_7, 'ngModel', currVal_0);
      }
      this._NgControlName_1_7.model = currVal_0 /* REF:package:corpus_ngdart/src/i94_ref_ng_form.html:25:43 */;
      changed = true;
      this._expr_0 = currVal_0;
    }
    if (changed) {
      this._NgControlName_1_7.ngAfterChanges();
    }
    this._textBinding_3.updateText(import19.interpolate0(local_f.valid)) /* REF:package:corpus_ngdart/src/i94_ref_ng_form.html:71:82 */;
  }

  @override
  void destroyInternal() {
    this._NgControlName_1_7.ngOnDestroy();
  }

  void _handleEvent_0($event) {
    this._DefaultValueAccessor_1_5.handleChange($event.target.value);
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.nome = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$I94RefNgForm, _debugComponentUrl));
      if (import10.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I94RefNgFormNgFactory = ComponentFactory<import1.I94RefNgForm>('i94-ref-ng-form', viewFactory_I94RefNgFormHost0);
ComponentFactory<import1.I94RefNgForm> get I94RefNgFormNgFactory {
  return _I94RefNgFormNgFactory;
}

ComponentFactory<import1.I94RefNgForm> createI94RefNgFormFactory() {
  return ComponentFactory('i94-ref-ng-form', viewFactory_I94RefNgFormHost0);
}

final List<Object> styles$I94RefNgFormHost = const [];

class _ViewI94RefNgFormHost0 extends import21.HostView<import1.I94RefNgForm> {
  @override
  void build() {
    this.componentView = ViewI94RefNgForm0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I94RefNgForm();
    this.initRootNode(_el_0);
  }
}

import21.HostView<import1.I94RefNgForm> viewFactory_I94RefNgFormHost0() {
  return _ViewI94RefNgFormHost0();
}
