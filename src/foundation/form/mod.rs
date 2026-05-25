use std::any::Any;
use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use parking_lot::Mutex;

use crate::foundation::binding::{Signal, State, TextController, ViewModelContext};
use crate::foundation::view_model::ValueCommand;

#[cfg(test)]
mod tests;

type SharedValue = Box<dyn Any>;
type FieldMap = BTreeMap<String, Arc<dyn RegisteredField>>;

#[derive(Clone)]
struct FormShared {
    inner: Arc<FormSharedInner>,
}

struct FormSharedInner {
    fields: Mutex<FieldMap>,
    errors: State<BTreeMap<String, Vec<String>>>,
}

impl FormShared {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            inner: Arc::new(FormSharedInner {
                fields: Mutex::new(BTreeMap::new()),
                errors: ctx.state(BTreeMap::new()),
            }),
        }
    }

    fn downgrade(&self) -> Weak<FormSharedInner> {
        Arc::downgrade(&self.inner)
    }

    fn register(&self, name: String, field: Arc<dyn RegisteredField>) {
        let mut fields = self.inner.fields.lock();
        if fields.contains_key(&name) {
            panic!("form field `{name}` is already registered");
        }
        fields.insert(name, field);
    }

    fn fields(&self) -> Vec<Arc<dyn RegisteredField>> {
        self.inner.fields.lock().values().cloned().collect()
    }

    fn errors(&self) -> Signal<BTreeMap<String, Vec<String>>> {
        self.inner.errors.signal()
    }

    fn is_valid(&self) -> Signal<bool> {
        self.inner.errors.project(BTreeMap::is_empty)
    }

    fn set_errors(&self, errors: BTreeMap<String, Vec<String>>) {
        self.inner.errors.set(errors);
    }
}

fn report_field_errors(form: &Weak<FormSharedInner>, name: &str, errors: Vec<String>) {
    if let Some(shared) = form.upgrade() {
        shared.errors.update(|all| {
            if errors.is_empty() {
                all.remove(name);
            } else {
                all.insert(name.to_string(), errors);
            }
        });
    }
}

trait RegisteredField: Send + Sync {
    fn validate_and_collect(&self) -> Vec<String>;
    fn reset_local(&self);
    fn clear_errors_local(&self);
    fn snapshot_value(&self) -> SharedValue;
    fn snapshot_errors(&self) -> Vec<String>;
    fn name(&self) -> &str;
}

/// 表示一组校验错误消息。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationErrors {
    errors: Vec<String>,
}

impl ValidationErrors {
    /// 创建一个空错误集合。
    pub fn none() -> Self {
        Self::default()
    }

    /// 创建一个只包含一条错误的集合。
    pub fn single(error: impl Into<String>) -> Self {
        Self {
            errors: vec![error.into()],
        }
    }

    /// 从多条错误消息构造错误集合。
    pub fn multiple<I, S>(errors: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            errors: errors.into_iter().map(Into::into).collect(),
        }
    }

    /// 返回当前是否没有错误。
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// 消费当前对象并返回底层错误列表。
    pub fn into_vec(self) -> Vec<String> {
        self.errors
    }
}

/// 纯 ViewModel 层的表单容器，负责字段注册、聚合、校验与错误同步。
#[derive(Clone)]
pub struct Form {
    context: ViewModelContext,
    shared: FormShared,
}

impl Form {
    /// 创建一个新的表单容器。
    pub fn new(ctx: &ViewModelContext) -> Self {
        Self {
            context: ctx.clone(),
            shared: FormShared::new(ctx),
        }
    }

    /// 注册一个通用字段。
    ///
    /// ```no_run
    /// use tgui::mvvm::{Form, ViewModelContext};
    /// use tgui::widgets::Checkbox;
    ///
    /// fn build_checkbox(ctx: &ViewModelContext) {
    ///     let form = Form::new(ctx);
    ///     let agree = form.field("agree", false);
    ///     let _checkbox = Checkbox::<()>::new(agree.signal()).on_change(agree.bind_change());
    /// }
    /// ```
    pub fn field<T>(&self, name: impl Into<String>, initial: T) -> FormField<T>
    where
        T: Clone + PartialEq + Send + Sync + 'static,
    {
        let name = name.into();
        let inner = Arc::new(FormFieldInner {
            name: name.clone(),
            state: self.context.state(initial.clone()),
            initial,
            validators: Mutex::new(Vec::new()),
            errors: self.context.state(Vec::new()),
            form: self.shared.downgrade(),
        });
        self.shared.register(name, inner.clone());
        FormField { inner }
    }

    /// 注册一个文本字段。
    ///
    /// ```no_run
    /// use tgui::mvvm::{Form, ViewModelContext};
    /// use tgui::widgets::Input;
    ///
    /// fn build_input(ctx: &ViewModelContext) {
    ///     let form = Form::new(ctx);
    ///     let email = form.text_field("email", "");
    ///     let _input = Input::<()>::new(email.controller());
    /// }
    /// ```
    pub fn text_field(
        &self,
        name: impl Into<String>,
        initial_text: impl Into<String>,
    ) -> TextFormField {
        let name = name.into();
        let initial = initial_text.into();
        let inner = Arc::new(TextFormFieldInner {
            name: name.clone(),
            controller: self.context.text_controller(initial.clone()),
            initial,
            validators: Mutex::new(Vec::new()),
            errors: self.context.state(Vec::new()),
            form: self.shared.downgrade(),
        });
        self.shared.register(name, inner.clone());
        TextFormField { inner }
    }

    /// 对所有已注册字段执行校验。
    pub fn validate(&self) -> bool {
        let mut aggregated = BTreeMap::new();
        for field in self.shared.fields() {
            let errors = field.validate_and_collect();
            if !errors.is_empty() {
                aggregated.insert(field.name().to_string(), errors);
            }
        }
        let is_valid = aggregated.is_empty();
        self.shared.set_errors(aggregated);
        is_valid
    }

    /// 先校验，再返回包含当前值和错误信息的快照。
    pub fn submit(&self) -> FormSnapshot {
        self.validate();
        self.snapshot()
    }

    /// 返回当前字段值和错误信息快照，不会主动触发校验。
    pub fn snapshot(&self) -> FormSnapshot {
        let mut values = BTreeMap::new();
        let mut errors = BTreeMap::new();
        for field in self.shared.fields() {
            let name = field.name().to_string();
            values.insert(name.clone(), field.snapshot_value());
            let field_errors = field.snapshot_errors();
            if !field_errors.is_empty() {
                errors.insert(name, field_errors);
            }
        }
        FormSnapshot { values, errors }
    }

    /// 将所有字段恢复到初始值并清空错误。
    pub fn reset(&self) {
        for field in self.shared.fields() {
            field.reset_local();
        }
        self.shared.set_errors(BTreeMap::new());
    }

    /// 清空所有字段错误。
    pub fn clear_errors(&self) {
        for field in self.shared.fields() {
            field.clear_errors_local();
        }
        self.shared.set_errors(BTreeMap::new());
    }

    /// 返回整个表单的有效性信号。
    pub fn is_valid(&self) -> Signal<bool> {
        self.shared.is_valid()
    }

    /// 返回按字段名索引的错误集合信号。
    pub fn errors(&self) -> Signal<BTreeMap<String, Vec<String>>> {
        self.shared.errors()
    }
}

type FieldValidator<T> = Arc<dyn Fn(&T) -> ValidationErrors + Send + Sync>;

struct FormFieldInner<T> {
    name: String,
    state: State<T>,
    initial: T,
    validators: Mutex<Vec<FieldValidator<T>>>,
    errors: State<Vec<String>>,
    form: Weak<FormSharedInner>,
}

impl<T> FormFieldInner<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    fn run_validation(&self) -> Vec<String> {
        let value = self.state.get();
        let validators = self.validators.lock().clone();
        let mut errors = Vec::new();
        for validator in validators {
            errors.extend(validator(&value).into_vec());
        }
        self.errors.set(errors.clone());
        report_field_errors(&self.form, &self.name, errors.clone());
        errors
    }
}

impl<T> RegisteredField for FormFieldInner<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    fn validate_and_collect(&self) -> Vec<String> {
        self.run_validation()
    }

    fn reset_local(&self) {
        self.state.set(self.initial.clone());
        self.errors.set(Vec::new());
        report_field_errors(&self.form, &self.name, Vec::new());
    }

    fn clear_errors_local(&self) {
        self.errors.set(Vec::new());
        report_field_errors(&self.form, &self.name, Vec::new());
    }

    fn snapshot_value(&self) -> SharedValue {
        Box::new(self.state.get())
    }

    fn snapshot_errors(&self) -> Vec<String> {
        self.errors.get()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 通用表单字段，适用于布尔、数值、枚举等非文本值。
#[derive(Clone)]
pub struct FormField<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    inner: Arc<FormFieldInner<T>>,
}

impl<T> FormField<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    /// 返回字段当前值的响应式信号。
    pub fn signal(&self) -> Signal<T> {
        self.inner.state.signal()
    }

    /// 读取当前值。
    pub fn get(&self) -> T {
        self.inner.state.get()
    }

    /// 设置字段值。
    pub fn set(&self, value: T) {
        self.inner.state.set(value);
    }

    /// 原地更新字段值。
    pub fn update<R>(&self, updater: impl FnOnce(&mut T) -> R) -> R {
        self.inner.state.update(updater)
    }

    /// 恢复字段初始值并清空当前错误。
    pub fn reset(&self) {
        self.inner.reset_local();
    }

    /// 追加一个校验器。
    pub fn validator(
        self,
        validator: impl Fn(&T) -> ValidationErrors + Send + Sync + 'static,
    ) -> Self {
        self.inner.validators.lock().push(Arc::new(validator));
        self
    }

    /// 执行当前字段校验。
    pub fn validate(&self) -> bool {
        self.inner.run_validation().is_empty()
    }

    /// 清空当前字段错误。
    pub fn clear_errors(&self) {
        self.inner.clear_errors_local();
    }

    /// 返回字段完整错误列表信号。
    pub fn errors(&self) -> Signal<Vec<String>> {
        self.inner.errors.signal()
    }

    /// 返回字段首条错误信号。
    pub fn first_error(&self) -> Signal<Option<String>> {
        self.inner.errors.project(|errors| errors.first().cloned())
    }

    /// 返回字段当前是否有效。
    pub fn is_valid(&self) -> Signal<bool> {
        self.inner.errors.project(Vec::is_empty)
    }

    /// 生成一个可直接绑定到 `on_change(...)` 的命令。
    pub fn bind_change<VM: 'static>(&self) -> ValueCommand<VM, T> {
        let state = self.inner.state.clone();
        ValueCommand::new(move |_: &mut VM, value| {
            state.set(value);
        })
    }
}

type TextFieldValidator = Arc<dyn Fn(&str) -> ValidationErrors + Send + Sync>;

struct TextFormFieldInner {
    name: String,
    controller: TextController,
    initial: String,
    validators: Mutex<Vec<TextFieldValidator>>,
    errors: State<Vec<String>>,
    form: Weak<FormSharedInner>,
}

impl TextFormFieldInner {
    fn run_validation(&self) -> Vec<String> {
        let text = self.controller.text();
        let validators = self.validators.lock().clone();
        let mut errors = Vec::new();
        for validator in validators {
            errors.extend(validator(&text).into_vec());
        }
        self.errors.set(errors.clone());
        report_field_errors(&self.form, &self.name, errors.clone());
        errors
    }
}

impl RegisteredField for TextFormFieldInner {
    fn validate_and_collect(&self) -> Vec<String> {
        self.run_validation()
    }

    fn reset_local(&self) {
        self.controller.set_text(self.initial.clone());
        self.errors.set(Vec::new());
        report_field_errors(&self.form, &self.name, Vec::new());
    }

    fn clear_errors_local(&self) {
        self.errors.set(Vec::new());
        report_field_errors(&self.form, &self.name, Vec::new());
    }

    fn snapshot_value(&self) -> SharedValue {
        Box::new(self.controller.text())
    }

    fn snapshot_errors(&self) -> Vec<String> {
        self.errors.get()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 文本表单字段，适用于 `Input` 和 `Textarea`。
#[derive(Clone)]
pub struct TextFormField {
    inner: Arc<TextFormFieldInner>,
}

impl TextFormField {
    /// 返回可供 `Input` / `Textarea` 绑定的文本控制器。
    pub fn controller(&self) -> TextController {
        self.inner.controller.clone()
    }

    /// 读取当前文本值。
    pub fn text(&self) -> String {
        self.inner.controller.text()
    }

    /// 直接设置文本值。
    pub fn set_text(&self, text: impl Into<String>) {
        self.inner.controller.set_text(text);
    }

    /// 恢复初始文本并清空错误。
    pub fn reset(&self) {
        self.inner.reset_local();
    }

    /// 追加一个文本校验器。
    pub fn validator(
        self,
        validator: impl Fn(&str) -> ValidationErrors + Send + Sync + 'static,
    ) -> Self {
        self.inner.validators.lock().push(Arc::new(validator));
        self
    }

    /// 执行当前字段校验。
    pub fn validate(&self) -> bool {
        self.inner.run_validation().is_empty()
    }

    /// 清空当前字段错误。
    pub fn clear_errors(&self) {
        self.inner.clear_errors_local();
    }

    /// 返回字段完整错误列表信号。
    pub fn errors(&self) -> Signal<Vec<String>> {
        self.inner.errors.signal()
    }

    /// 返回字段首条错误信号。
    ///
    /// ```no_run
    /// use tgui::mvvm::{Form, ViewModelContext};
    /// use tgui::widgets::Text;
    ///
    /// fn build_error_text(ctx: &ViewModelContext) {
    ///     let form = Form::new(ctx);
    ///     let email = form.text_field("email", "");
    ///     let _error = Text::new(email.first_error().map(|value| value.unwrap_or_default()));
    /// }
    /// ```
    pub fn first_error(&self) -> Signal<Option<String>> {
        self.inner.errors.project(|errors| errors.first().cloned())
    }

    /// 返回字段当前是否有效。
    pub fn is_valid(&self) -> Signal<bool> {
        self.inner.errors.project(Vec::is_empty)
    }
}

/// 表单值与错误信息快照。
pub struct FormSnapshot {
    values: BTreeMap<String, SharedValue>,
    errors: BTreeMap<String, Vec<String>>,
}

impl FormSnapshot {
    /// 返回整个快照当前是否有效。
    pub fn is_valid(&self) -> bool {
        self.errors.values().all(Vec::is_empty)
    }

    /// 按字段名和目标类型读取值。
    pub fn get<T>(&self, name: &str) -> Option<T>
    where
        T: Clone + 'static,
    {
        self.values
            .get(name)
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
    }

    /// 返回指定字段的错误切片。
    pub fn errors_for(&self, name: &str) -> Option<&[String]> {
        self.errors.get(name).map(Vec::as_slice)
    }
}
