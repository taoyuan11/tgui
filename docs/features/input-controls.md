# 表单增强控件

这一组组件建立在 `TextController`、`Popover`、原生对话框和受控状态之上，适合把日期、时间、数字、颜色和文件选择纳入 MVVM 表单。它们的共同模式是：组件读取 `State` / `Signal` / `TextController`，用户交互触发 `ValueCommand`，ViewModel 在回调中更新受控状态。

## 组件清单

| 组件 | 构造方式 | 主要状态 | 事件 payload |
| --- | --- | --- | --- |
| `Calendar` | `Calendar::new(display_month, selected)` | 当前展示月份、选中日期 | `CalendarSelectionChange` |
| `DatePicker` | `DatePicker::new(controller, selected, display_month)` | 输入文本、选中日期、弹层开闭、展示月份 | `DatePickerChange` |
| `TimePicker` | `TimePicker::new(controller, selected)` | 输入文本、选中时间、弹层开闭 | `TimePickerChange` |
| `NumberInput` | `NumberInput::new(controller, value)` | 输入文本、解析后的数字 | `NumberInputChange` |
| `ColorPicker` | `ColorPicker::new(color)` | 当前颜色、弹层开闭 | `ColorPickerChange` |
| `Upload` | `Upload::new(files)` | 文件队列 | `UploadSelection`、`UploadRemove` |

这些控件都支持 `disable(...)`、`style(...)` 和 `style_full(...)`。`DatePicker`、`TimePicker`、`NumberInput` 还支持 `validation(...)`，可直接和表单校验视觉状态配合。

## DatePicker

`DatePicker` 由三个受控值组成：

- `TextController`：保存输入框文本、选区和 IME composition。
- `selected: Option<NaiveDate>`：已解析并选中的日期。
- `display_month: NaiveDate`：日历弹层正在展示的月份。

```rust
DatePicker::new(
    self.date_text.clone(),
    self.selected_date.signal(),
    self.display_month.signal(),
)
.open(self.date_open.signal())
.placeholder("YYYY-MM-DD")
.validation(self.date_validation.signal())
.on_open_change(ValueCommand::new(|vm: &mut FormVm, open| {
    vm.date_open.set(open);
}))
.on_month_change(ValueCommand::new(|vm: &mut FormVm, month| {
    vm.display_month.set(month);
}))
.on_change(ValueCommand::new(|vm: &mut FormVm, change: DatePickerChange| {
    vm.selected_date.set(change.date);
    vm.date_text.set_text(change.text);
}))
```

`DatePickerChange` 字段：

| 字段 | 说明 |
| --- | --- |
| `date: Option<NaiveDate>` | 按 `%Y-%m-%d` 解析成功时为 `Some(date)`，手动输入无法解析时为 `None`。 |
| `text: String` | 用户输入或日历选择产生的原始文本。 |

常见校验策略是：允许用户继续输入，但当 `text` 非空且 `date == None` 时把 `validation` 设为 invalid，并在旁边展示错误文本。

## TimePicker

`TimePicker` 同样由 `TextController` 驱动，`minute_step(...)` 控制弹层分钟候选的步进。手动输入会尝试解析常见时间格式，选择弹层项会写回规范化文本。

```rust
TimePicker::new(self.time_text.clone(), self.selected_time.signal())
    .open(self.time_open.signal())
    .minute_step(15)
    .placeholder("HH:MM")
    .on_open_change(ValueCommand::new(|vm: &mut FormVm, open| {
        vm.time_open.set(open);
    }))
    .on_change(ValueCommand::new(|vm: &mut FormVm, change: TimePickerChange| {
        vm.selected_time.set(change.time);
        vm.time_text.set_text(change.text);
    }))
```

`TimePickerChange` 字段：

| 字段 | 说明 |
| --- | --- |
| `time: Option<NaiveTime>` | 解析成功时的时间。 |
| `text: String` | 当前输入框文本。 |

`minute_step` 会被限制在 `1..=60`。如果业务要求只能选择整点、半小时或十五分钟，优先在组件上设置步进，再在 ViewModel 里做最终校验。

## NumberInput

`NumberInput` 组合文本输入和两个步进按钮。它不会替应用保存业务值，而是在 `on_change` 中把解析结果发回 ViewModel。

```rust
NumberInput::new(self.quantity_text.clone(), self.quantity.signal())
    .range(0.0, 99.0)
    .step(1.0)
    .placeholder("0")
    .on_change(ValueCommand::new(|vm: &mut FormVm, change: NumberInputChange| {
        vm.quantity.set(change.value);
        vm.quantity_text.set_text(change.text);
    }))
```

常用 API：

| API | 说明 |
| --- | --- |
| `min(value)` / `max(value)` | 单独设置下限或上限。 |
| `range(min, max)` | 同时设置上下限。 |
| `step(value)` | 设置点击加减按钮时的步长；非法值会回退为 `1.0`。 |
| `placeholder(text)` | 设置空值占位符。 |
| `validation(state)` | 绑定校验视觉状态。 |

`NumberInputChange` 字段：

| 字段 | 说明 |
| --- | --- |
| `value: Option<f64>` | 解析并通过范围裁剪后的值；输入为空或非法时为 `None`。 |
| `text: String` | 当前文本。 |
| `trigger: NumberInputChangeTrigger` | `Text`、`StepUp` 或 `StepDown`。 |

## ColorPicker

`ColorPicker` 使用输入控件风格的触发器展示当前颜色，弹层内提供预设色和 RGBA 通道调整。

```rust
ColorPicker::new(self.accent.signal())
    .open(self.color_open.signal())
    .swatches(vec![
        Color::hexa(0x2563EBFF),
        Color::hexa(0x16A34AFF),
        Color::hexa(0xDC2626FF),
    ])
    .on_open_change(ValueCommand::new(|vm: &mut FormVm, open| {
        vm.color_open.set(open);
    }))
    .on_change(ValueCommand::new(|vm: &mut FormVm, change: ColorPickerChange| {
        vm.accent.set(change.color);
    }))
```

`ColorPickerChangeTrigger` 可用于区分来源：`Swatch`、`Red`、`Green`、`Blue`、`Alpha`。如果需要在拖动通道时做节流上传，可以根据 `trigger` 决定是否立即持久化。

## Upload

`Upload` 是受控文件队列组件，本身不执行 HTTP 上传。它负责展示 drop zone、文件列表、进度、完成态、错误态和删除按钮；真正的上传、取消、重试和状态推进由 ViewModel 或业务服务完成。

```rust
Upload::new(self.upload_files.signal())
    .title("拖放文件到这里")
    .hint("支持 PNG、JPG、PDF，单文件不超过 10 MB")
    .accept_extensions(&["png", "jpg", "pdf"])
    .max_files(8)
    .max_file_size(10 * 1024 * 1024)
    .on_select(ValueCommand::new(|vm: &mut FormVm, selection: UploadSelection| {
        vm.upload_errors.set(
            selection
                .rejected
                .iter()
                .map(|item| item.reason.clone())
                .collect(),
        );
        vm.upload_files.update(|files| files.extend(selection.files));
    }))
    .on_remove(ValueCommand::new(|vm: &mut FormVm, remove: UploadRemove| {
        vm.upload_files
            .update(|files| files.retain(|file| file.id != remove.id));
    }))
```

相关类型：

| 类型 | 说明 |
| --- | --- |
| `UploadFile` | 文件项，包含 `id`、`path`、`name`、`size_bytes` 和 `status`。 |
| `UploadStatus` | `Queued`、`Uploading { progress }`、`Complete`、`Error(message)`。 |
| `UploadSelection` | 选择或拖放后的结果，包含通过校验的 `files` 和被拒绝的 `rejected`。 |
| `UploadRejection` | 被拒绝文件的 `path` 和 `reason`。 |
| `UploadRemove` | 删除事件，包含要移除的 `UploadFileId`。 |

上传进度更新示例：

```rust
fn set_upload_progress(&mut self, id: UploadFileId, progress: f32) {
    self.upload_files.update(|files| {
        if let Some(file) = files.iter_mut().find(|file| file.id == id) {
            file.status = UploadStatus::Uploading { progress };
        }
    });
}
```

## Calendar

需要直接展示日历面板时使用 `Calendar`。它适合仪表盘、日期范围选择器或自定义 picker。

```rust
Calendar::new(self.display_month.signal(), self.selected_date.signal())
    .today(Some(chrono::Local::now().date_naive()))
    .on_change(ValueCommand::new(|vm: &mut FormVm, change: CalendarSelectionChange| {
        vm.display_month.set(change.display_month);
        if matches!(change.trigger, CalendarChangeTrigger::Day | CalendarChangeTrigger::Today) {
            vm.selected_date.set(Some(change.date));
        }
    }))
```

`CalendarSelectionChange` 字段：

| 字段 | 说明 |
| --- | --- |
| `date: NaiveDate` | 当前选择或跳转对应的日期。 |
| `display_month: NaiveDate` | 变更后应该展示的月份。 |
| `trigger: CalendarChangeTrigger` | `Day`、`PreviousMonth`、`NextMonth` 或 `Today`。 |

## 表单组织建议

- 文本类 picker 使用 `TextController` 保存文本，不要只保存 `Option<NaiveDate>` 或 `Option<f64>`。
- `open` 状态需要外部控制时绑定 `State<bool>`，否则可让组件使用内部开闭状态。
- 上传组件只管理 UI 队列，不要在组件回调里阻塞执行真实上传。
- 校验状态放在 ViewModel 中，由 `validation(...)` 传给控件；错误说明用普通 `Text` 或 `FormField` 组合展示。
