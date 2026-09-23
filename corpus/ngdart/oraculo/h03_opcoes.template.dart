// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'h03_opcoes.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'h03_opcoes.dart' as import1;
import 'package:ngforms/src/directives/select_control_value_accessor.dart' as import2;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import3;
import 'package:ngforms/src/directives/ng_model.dart' as import4;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import6;
import 'package:ngforms/src/directives/validators.template.dart' as import7;
import 'dart:core';
import 'package:ngforms/src/directives/default_value_accessor.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import11;
import 'package:ngdart/src/core/linker/views/view.dart' as import12;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import13;
import 'package:ngdart/src/utilities.dart' as import14;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import15;
import 'package:ngdart/src/devtools.dart' as import16;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngforms/src/directives/validators.dart' as import18;
import 'package:ngdart/src/meta/di_tokens.dart' as import19;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import20;
import 'package:ngforms/src/directives/ng_control.dart' as import21;
import 'package:ngdart/src/runtime/check_binding.dart' as import22;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import24;
import 'package:ngdart/src/runtime/text_binding.dart' as import25;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import26;
import 'package:ngdart/src/runtime/interpolate.dart' as import27;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import28;

final List<Object> styles$H03Opcoes = const [];

class ViewH03Opcoes0 extends import0.ComponentView<import1.H03Opcoes> {
  late final import2.SelectControlValueAccessor _SelectControlValueAccessor_0_5;
  late final List<import3.ControlValueAccessor<dynamic>> _NgValueAccessor_0_6;
  late final import4.NgModel _NgModel_0_7;
  late final ViewContainer _appEl_1;
  late final import6.NgFor _NgFor_1_9;
  late final import7.MaxLengthValidatorNgCd _MaxLengthValidator_3_5;
  late final List<Object> _NgValidators_3_6;
  late final import9.DefaultValueAccessor _DefaultValueAccessor_3_7;
  late final List<import3.ControlValueAccessor<dynamic>> _NgValueAccessor_3_8;
  late final import4.NgModel _NgModel_3_9;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_3;
  late final import10.InputElement _el_3;
  static import11.ComponentStyles? _componentStyles;
  ViewH03Opcoes0(import12.View parentView, int parentIndex) : super(parentView, parentIndex, import13.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import14.unsafeCast(import10.document.createElement('h03-opcoes'));
  }
  static String? get _debugComponentUrl {
    return (import14.isDevMode ? 'asset:corpus_ngdart/lib/src/h03_opcoes.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import10.document;
    final _el_0 = import15.appendElement<import10.SelectElement>(doc, parentRenderNode, 'select');
    this._SelectControlValueAccessor_0_5 = import2.SelectControlValueAccessor(_el_0);
    this._NgValueAccessor_0_6 = [this._SelectControlValueAccessor_0_5];
    this._NgModel_0_7 = import4.NgModel(null, this._NgValueAccessor_0_6);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(_el_0, this._SelectControlValueAccessor_0_5);
      import16.Inspector.instance.registerDirective(_el_0, this._NgModel_0_7);
    }
    final _anchor_1 = import15.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_H03Opcoes1);
    this._NgFor_1_9 = import6.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    final _text_2 = import15.appendText(parentRenderNode, '\n');
    this._el_3 = import15.appendElement<import10.InputElement>(doc, parentRenderNode, 'input');
    import15.setAttribute(this._el_3, 'style', 'width: 10px;');
    this._el_3.tabIndex = 2;
    this._MaxLengthValidator_3_5 = import7.MaxLengthValidatorNgCd(import18.MaxLengthValidator());
    this._NgValidators_3_6 = [this._MaxLengthValidator_3_5.instance];
    this._DefaultValueAccessor_3_7 = import9.DefaultValueAccessor(this._el_3);
    this._NgValueAccessor_3_8 = [this._DefaultValueAccessor_3_7];
    this._NgModel_3_9 = import4.NgModel(this._NgValidators_3_6, this._NgValueAccessor_3_8);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(this._el_3, this._MaxLengthValidator_3_5.instance);
      import16.Inspector.instance.registerDirective(this._el_3, this._DefaultValueAccessor_3_7);
      import16.Inspector.instance.registerDirective(this._el_3, this._NgModel_3_9);
    }
    _el_0.addEventListener('blur', this.eventHandler0(this._SelectControlValueAccessor_0_5.touchHandler));
    _el_0.addEventListener('change', this.eventHandler1(this._handleEvent_0));
    final subscription_0 = this._NgModel_0_7.update.listen(this.eventHandler1(this._handleEvent_1));
    this._el_3.addEventListener('blur', this.eventHandler0(this._DefaultValueAccessor_3_7.touchHandler));
    this._el_3.addEventListener('input', this.eventHandler1(this._handleEvent_2));
    final subscription_1 = this._NgModel_3_9.update.listen(this.eventHandler1(this._handleEvent_3));
    this.initSubscriptions([subscription_0, subscription_1]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 1)) {
      if (identical(token, import2.SelectControlValueAccessor)) {
        return this._SelectControlValueAccessor_0_5;
      }
      if (identical(token, const import19.MultiToken<import20.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
        return this._NgValueAccessor_0_6;
      }
      if ((identical(token, import4.NgModel) || identical(token, import21.NgControl))) {
        return this._NgModel_0_7;
      }
    }
    if ((3 == nodeIndex)) {
      if (identical(token, const import19.MultiToken<Object>('NgValidators'))) {
        return this._NgValidators_3_6;
      }
      if (identical(token, const import19.MultiToken<import20.ControlValueAccessor<dynamic>>('NgValueAccessor'))) {
        return this._NgValueAccessor_3_8;
      }
      if ((identical(token, import4.NgModel) || identical(token, import21.NgControl))) {
        return this._NgModel_3_9;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    final currVal_0 = _ctx.escolha;
    if (import22.checkBinding(this._expr_0, currVal_0, 'escolha', 'package:corpus_ngdart/src/h03_opcoes.html')) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._NgModel_0_7, 'ngModel', currVal_0);
      }
      this._NgModel_0_7.model = currVal_0 /* REF:package:corpus_ngdart/src/h03_opcoes.html:8:29 */;
      changed = true;
      this._expr_0 = currVal_0;
    }
    if (changed) {
      this._NgModel_0_7.ngAfterChanges();
    }
    if (((!import22.debugThrowIfChanged) && firstCheck)) {
      this._NgModel_0_7.ngOnInit();
    }
    final currVal_1 = _ctx.opcoes;
    if (import22.checkBinding(this._expr_1, currVal_1, 'opcoes', 'package:corpus_ngdart/src/h03_opcoes.html')) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_1);
      }
      this._NgFor_1_9.ngForOf = currVal_1 /* REF:package:corpus_ngdart/src/h03_opcoes.html:43:67 */;
      this._expr_1 = currVal_1;
    }
    if ((!import22.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    if (firstCheck) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._MaxLengthValidator_3_5.instance, 'maxlength', 5);
      }
      this._MaxLengthValidator_3_5.instance.maxlength = 5 /* REF:package:corpus_ngdart/src/h03_opcoes.html:136:151 */;
    }
    changed = false;
    final currVal_3 = _ctx.escolha;
    if (import22.checkBinding(this._expr_3, currVal_3, 'escolha', 'package:corpus_ngdart/src/h03_opcoes.html')) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._NgModel_3_9, 'ngModel', currVal_3);
      }
      this._NgModel_3_9.model = currVal_3 /* REF:package:corpus_ngdart/src/h03_opcoes.html:114:135 */;
      changed = true;
      this._expr_3 = currVal_3;
    }
    if (changed) {
      this._NgModel_3_9.ngAfterChanges();
    }
    if (((!import22.debugThrowIfChanged) && firstCheck)) {
      this._NgModel_3_9.ngOnInit();
    }
    this._appEl_1.detectChangesInNestedViews();
    this._MaxLengthValidator_3_5.detectHostChanges(this, this._el_3);
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }

  void _handleEvent_0($event) {
    this._SelectControlValueAccessor_0_5.handleChange($event.target.value);
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.escolha = $event;
  }

  void _handleEvent_2($event) {
    this._DefaultValueAccessor_3_7.handleChange($event.target.value);
  }

  void _handleEvent_3($event) {
    final _ctx = this.ctx;
    _ctx.escolha = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import11.ComponentStyles.unscoped(styles$H03Opcoes, _debugComponentUrl));
      if (import14.isDevMode) {
        import11.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _H03OpcoesNgFactory = ComponentFactory<import1.H03Opcoes>('h03-opcoes', viewFactory_H03OpcoesHost0);
ComponentFactory<import1.H03Opcoes> get H03OpcoesNgFactory {
  return _H03OpcoesNgFactory;
}

ComponentFactory<import1.H03Opcoes> createH03OpcoesFactory() {
  return ComponentFactory('h03-opcoes', viewFactory_H03OpcoesHost0);
}

class _ViewH03Opcoes1 extends import24.EmbeddedView<import1.H03Opcoes> {
  final import25.TextBinding _textBinding_1 = import25.TextBinding();
  late final import2.NgSelectOption _NgSelectOption_0_5;
  Object? _expr_0;
  _ViewH03Opcoes1(import26.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import14.unsafeCast(doc.createElement('option'));
    this._NgSelectOption_0_5 = import2.NgSelectOption(_el_0, import14.unsafeCast<ViewH03Opcoes0>((this.parentView!))._SelectControlValueAccessor_0_5);
    if (import16.isDevToolsEnabled) {
      import16.Inspector.instance.registerDirective(_el_0, this._NgSelectOption_0_5);
    }
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_o = import14.unsafeCast<String>(this.locals['\$implicit']);
    final currVal_0 = local_o;
    if (import22.checkBinding(this._expr_0, currVal_0, 'o', 'package:corpus_ngdart/src/h03_opcoes.html')) {
      if (import16.isDevToolsEnabled) {
        import16.Inspector.instance.recordInput(this._NgSelectOption_0_5, 'ngValue', currVal_0);
      }
      this._NgSelectOption_0_5.ngValue = currVal_0 /* REF:package:corpus_ngdart/src/h03_opcoes.html:68:81 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import27.interpolateString0(local_o)) /* REF:package:corpus_ngdart/src/h03_opcoes.html:82:87 */;
  }

  @override
  void destroyInternal() {
    this._NgSelectOption_0_5.ngOnDestroy();
  }
}

import24.EmbeddedView<void> viewFactory_H03Opcoes1(import26.RenderView parentView, int parentIndex) {
  return _ViewH03Opcoes1(parentView, parentIndex);
}

final List<Object> styles$H03OpcoesHost = const [];

class _ViewH03OpcoesHost0 extends import28.HostView<import1.H03Opcoes> {
  @override
  void build() {
    this.componentView = ViewH03Opcoes0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.H03Opcoes();
    this.initRootNode(_el_0);
  }
}

import28.HostView<import1.H03Opcoes> viewFactory_H03OpcoesHost0() {
  return _ViewH03OpcoesHost0();
}
