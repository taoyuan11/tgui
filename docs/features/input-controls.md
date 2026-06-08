# 表单增强控件

这一组组件建立在 `TextController`、`Popover`、原生对话框和受控状态之上，适合把日期、时间、数字、颜色和文件选择纳入 MVVM 表单。

## 组件清单

| 组件 | 用途 |
| --- | --- |
| `Calendar` | 独立日历面板，直接展示月份网格和选中日期。 |
| `DatePicker` | 输入框 + 日历弹层，支持手动输入和日期选择。 |
| `TimePicker` | 输入框 + 时间选项弹层，支持固定分钟步进。 |
| `NumberInput` | 文本数字输入 + 步进按钮，支持范围限制。 |
| `ColorPicker` | 色块触发器 + 色板/RGBA 通道弹层。 |
| `Upload` | 受控文件队列，支持点击选择、拖放、移除、进度和错误状态。 |

## DatePicker

`DatePicker` 使用 `TextController` 保存输入文本，并通过 `selected` 和 `display_month` 两个受控值同步日期与当前月份。

```rust
DatePicker::new(
    app.demo_date_text.clone(),
    app.demo_date.signal(),
    app.demo_date_month.signal(),
)
.open(app.demo_date_open.signal())
.on_open_change(ValueCommand::new(App::set_demo_date_open))
.on_month_change(ValueCommand::new(App::set_demo_date_month))
.on_change(ValueCommand::new(App::set_demo_date))
```

`DatePickerChange` 包含解析后的 `date: Option<NaiveDate>` 和原始 `text`。手动输入无法解析时，`date` 为 `None`，调用方可以用表单校验状态提示用户。

## TimePicker

`TimePicker` 同样由 `TextController` 驱动。`minute_step(...)` 控制弹层中的分钟选项步进。

```rust
TimePicker::new(app.demo_time_text.clone(), app.demo_time.signal())
    .open(app.demo_time_open.signal())
    .minute_step(30)
    .on_open_change(ValueCommand::new(App::set_demo_time_open))
    .on_change(ValueCommand::new(App::set_demo_time))
```

`TimePickerChange` 包含 `time: Option<NaiveTime>` 和原始 `text`。

## NumberInput

`NumberInput` 组合文本输入和两侧步进按钮。`range(...)` 同时设置最小值和最大值，`step(...)` 设置按钮调整步长。

```rust
NumberInput::new(app.demo_number_text.clone(), app.demo_number.signal())
    .range(0.0, 99.0)
    .step(1.0)
    .on_change(ValueCommand::new(App::set_demo_number))
```

`NumberInputChangeTrigger` 会区分文本输入、向上步进和向下步进，便于 ViewModel 做不同反馈。

## ColorPicker

`ColorPicker` 使用输入控件风格的触发器展示当前颜色，弹层内提供预设色和 RGBA 通道调整。

```rust
ColorPicker::new(app.demo_color.signal())
    .open(app.demo_color_open.signal())
    .on_open_change(ValueCommand::new(App::set_demo_color_open))
    .on_change(ValueCommand::new(App::set_demo_color))
```

`swatches(...)` 可以替换默认预设色。`ColorPickerChangeTrigger` 会标识变化来自色板还是某个颜色通道。

## Upload

`Upload` 是受控文件队列组件，本身不执行 HTTP 上传。它负责展示 drop zone、文件列表、进度、完成态、错误态和删除按钮。

```rust
Upload::new(app.upload_files.signal())
    .accept_extensions(&["png", "jpg", "pdf", "txt"])
    .max_files(8)
    .max_file_size(10 * 1024 * 1024)
    .on_select(ValueCommand::new(App::add_upload_files))
    .on_remove(ValueCommand::new(App::remove_upload_file))
```

选择或拖放文件后会产生 `UploadSelection`，其中 `files` 是通过校验的 `UploadFile`，`rejected` 包含被拒绝文件和原因。调用方负责把结果写回 `State<Vec<UploadFile>>`，并按真实上传进度更新 `UploadStatus`。

## Calendar

需要直接展示日历面板时使用 `Calendar`：

```rust
Calendar::new(app.demo_date_month.signal(), app.demo_date.signal())
    .on_change(ValueCommand::new(|app: &mut App, change| {
        app.demo_date.set(Some(change.date));
        app.demo_date_month.set(change.display_month);
    }))
```

`CalendarChangeTrigger` 可区分选择日期、上一月、下一月和回到今天。

## 样式与校验

这些控件都支持 `style(...)` / `style_full(...)`。`DatePicker`、`TimePicker` 和 `NumberInput` 还支持 `validation(...)`，可以直接复用 `FormField` 或 `TextFormField` 的视觉校验状态。
