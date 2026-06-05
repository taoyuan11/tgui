use std::path::PathBuf;

use crate::navigation::DemoPage;
use crate::{navigation, pages, styles};
use tgui::prelude::*;

#[derive(Clone)]
pub(crate) struct DemoContact {
    pub name: &'static str,
    pub role: &'static str,
    pub status: &'static str,
}

impl DemoContact {
    pub(crate) fn new(name: &'static str, role: &'static str, status: &'static str) -> Self {
        Self { name, role, status }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Employee {
    pub id: &'static str,
    pub name: String,
    pub role: String,
    pub team: String,
    pub status: String,
}

impl Employee {
    fn new(
        id: &'static str,
        name: &'static str,
        role: &'static str,
        team: &'static str,
        status: &'static str,
    ) -> Self {
        Self {
            id,
            name: name.to_string(),
            role: role.to_string(),
            team: team.to_string(),
            status: status.to_string(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct VideoPlayer {
    pub controller: VideoController,
    pub source: TextController,
    pub status: State<String>,
}

impl VideoPlayer {
    fn new(context: &ViewModelContext) -> Self {
        let controller = VideoController::new(context);
        controller.set_volume(0.0);
        Self {
            controller,
            source: context.text_controller(""),
            status: context.state("尚未加载视频。请输入文件路径或 URL。".to_string()),
        }
    }

    pub(crate) fn playback_status(&self) -> Signal<String> {
        self.controller.playback_state().map(video_status_text)
    }

    pub(crate) fn load_from_input(&mut self) {
        let source = self.source.text();
        let source = source.trim();
        if source.is_empty() {
            self.status
                .set("请输入视频文件路径或 URL 后再加载。".to_string());
            return;
        }

        let video_source = if source.starts_with("http://") || source.starts_with("https://") {
            VideoSource::url(source.to_string())
        } else {
            VideoSource::File(PathBuf::from(source))
        };

        match self.controller.load(video_source) {
            Ok(()) => self.status.set(format!("已请求加载视频: {source}")),
            Err(error) => {
                let message = format!("加载视频失败: {error}");
                tgui_log(LogLevel::Error, &message);
                self.status.set(message);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct App {
    pub current_page: State<DemoPage>,
    expanded_code: State<Vec<&'static str>>,

    pub theme: State<ThemeMode>,
    pub reduced_motion: State<bool>,
    pub switch: State<bool>,
    pub checkbox: State<bool>,
    pub radio: State<bool>,
    pub slider_value: State<f32>,
    pub contact_method: State<String>,
    pub select_action: State<Option<String>>,

    pub notification_status: State<String>,
    pub toast_status: State<String>,

    pub popover_open: State<bool>,
    pub popover_switch: State<bool>,
    pub popover_note: TextController,

    pub input_text: TextController,
    pub textarea_text: TextController,
    pub audio_status: State<String>,
    pub audio_controller: AudioController,
    pub video_player: VideoPlayer,

    pub toast_queue: ToastQueue<App>,
    pub toast_top_start: ToastQueue<App>,
    pub toast_top_center: ToastQueue<App>,
    pub toast_top_end: ToastQueue<App>,
    pub toast_bottom_start: ToastQueue<App>,
    pub toast_bottom_center: ToastQueue<App>,

    pub profile_form: Form,
    pub profile_name: TextFormField,
    pub profile_email: TextFormField,
    pub profile_newsletter: FormField<bool>,
    pub profile_status: State<String>,

    pub tabs_selected: State<String>,
    pub tabs_order: State<Vec<String>>,
    pub tabs_reorder_status: State<String>,

    pub drawer_left_open: State<bool>,
    pub drawer_right_open: State<bool>,
    pub drawer_top_open: State<bool>,
    pub drawer_bottom_open: State<bool>,
    pub drawer_push_open: State<bool>,

    pub modal_alert_open: State<bool>,
    pub modal_confirm_open: State<bool>,
    pub modal_confirm_result: State<String>,
    pub modal_form_open: State<bool>,
    pub modal_form_name: TextController,
    pub modal_form_result: State<String>,

    pub list_selected_keys: State<Vec<WidgetKey>>,
    pub list_loading: State<bool>,
    pub list_show_empty: State<bool>,
    pub list_status: State<String>,
    pub virtual_rows: Vec<String>,

    pub data_rows: State<Vec<Employee>>,
    pub data_selected: State<Vec<WidgetKey>>,
    pub data_sort: State<Vec<DataGridSort>>,
    pub data_column_order: State<Vec<String>>,
    pub data_name_width: State<Dp>,
    pub data_role_width: State<Dp>,
    pub data_team_width: State<Dp>,
    pub data_status_width: State<Dp>,
    pub data_status: State<String>,
}

impl ViewModel for App {
    fn new(context: &ViewModelContext) -> Self {
        let audio = AudioController::new(context);
        audio.set_volume(0.8);

        let profile_form = Form::new(context);
        let profile_name = profile_form
            .text_field("name", "Alice Wonderland")
            .validator(|value| {
                if value.trim().is_empty() {
                    ValidationErrors::single("名称不能为空")
                } else {
                    ValidationErrors::none()
                }
            })
            .async_validator(|value| {
                if value.eq_ignore_ascii_case("admin") {
                    ValidationErrors::single("该名称已被保留")
                } else {
                    ValidationErrors::none()
                }
            });
        let profile_email = profile_form
            .text_field("email", "alice@example.com")
            .validator(|value| {
                if value.contains('@') {
                    ValidationErrors::none()
                } else {
                    ValidationErrors::single("请输入有效邮箱")
                }
            })
            .async_validator(|value| {
                if value.ends_with("@example.com") {
                    ValidationErrors::none()
                } else {
                    ValidationErrors::single("仅示例邮箱域名可通过异步校验")
                }
            });
        let profile_newsletter = profile_form.field("newsletter", true).validator(|enabled| {
            if *enabled {
                ValidationErrors::none()
            } else {
                ValidationErrors::single("建议至少订阅一项")
            }
        });

        Self {
            current_page: context.state(DemoPage::Basics),
            expanded_code: context.state(Vec::new()),
            theme: context.state(ThemeMode::System),
            reduced_motion: context.state(false),
            switch: context.state(false),
            checkbox: context.state(false),
            radio: context.state(false),
            slider_value: context.state(80.0),
            contact_method: context.state(String::from("system")),
            select_action: context.state(None),
            notification_status: context.state(String::from("尚未发送通知")),
            toast_status: context.state(String::from("尚未触发 Toast 操作")),
            popover_open: context.state(false),
            popover_switch: context.state(true),
            popover_note: context.text_controller("预览状态下也可以直接编辑这里的内容。"),
            input_text: context.text_controller(""),
            textarea_text: context.text_controller(
                "这是一个受控 Textarea。\n你可以在这里输入多行内容，示例不会保存修改。",
            ),
            audio_status: context.state("尚未加载音频。请输入文件路径或 URL。".to_string()),
            audio_controller: audio,
            video_player: VideoPlayer::new(context),
            toast_queue: ToastQueue::new(context),
            toast_top_start: ToastQueue::new(context),
            toast_top_center: ToastQueue::new(context),
            toast_top_end: ToastQueue::new(context),
            toast_bottom_start: ToastQueue::new(context),
            toast_bottom_center: ToastQueue::new(context),
            profile_form,
            profile_name,
            profile_email,
            profile_newsletter,
            profile_status: context.state("表单尚未提交".to_string()),
            tabs_selected: context.state("overview".to_string()),
            tabs_order: context.state(vec![
                "overview".to_string(),
                "settings".to_string(),
                "logs".to_string(),
                "metrics".to_string(),
                "advanced".to_string(),
            ]),
            tabs_reorder_status: context.state("尚未重排".to_string()),
            drawer_left_open: context.state(false),
            drawer_right_open: context.state(false),
            drawer_top_open: context.state(false),
            drawer_bottom_open: context.state(false),
            drawer_push_open: context.state(false),
            modal_alert_open: context.state(false),
            modal_confirm_open: context.state(false),
            modal_confirm_result: context.state("尚未触发 confirm".to_string()),
            modal_form_open: context.state(false),
            modal_form_name: context.text_controller(""),
            modal_form_result: context.state("尚未提交".to_string()),
            list_selected_keys: context.state(vec![WidgetKey::from("ana")]),
            list_loading: context.state(false),
            list_show_empty: context.state(false),
            list_status: context
                .state("点击行、Shift 选择范围、按 Enter 或右键打开菜单。".to_string()),
            virtual_rows: (0..10_000)
                .map(|index| format!("Log row #{index:04} - virtualized data item"))
                .collect(),
            data_rows: context.state(vec![
                Employee::new(
                    "e-001",
                    "Ada Torres",
                    "Product Lead",
                    "Platform",
                    "Planning",
                ),
                Employee::new("e-002", "Mika Chen", "Designer", "Experience", "Reviewing"),
                Employee::new(
                    "e-003",
                    "Nora Patel",
                    "Researcher",
                    "Experience",
                    "Interviewing",
                ),
                Employee::new(
                    "e-004",
                    "Owen Blake",
                    "Runtime Engineer",
                    "Core",
                    "Implementing",
                ),
                Employee::new("e-005", "Li Wei", "Rendering Engineer", "Core", "Profiling"),
                Employee::new(
                    "e-006",
                    "Sam Rivera",
                    "Platform Engineer",
                    "Desktop",
                    "Testing",
                ),
                Employee::new(
                    "e-007",
                    "Iris Morgan",
                    "QA Engineer",
                    "Desktop",
                    "Validating",
                ),
                Employee::new("e-008", "Jun Park", "Data Engineer", "Tools", "Shipping"),
            ]),
            data_selected: context.state(vec![WidgetKey::from("e-001")]),
            data_sort: context.state(Vec::new()),
            data_column_order: context.state(vec![
                "id".to_string(),
                "name".to_string(),
                "role".to_string(),
                "team".to_string(),
                "status".to_string(),
            ]),
            data_name_width: context.state(dp(190.0)),
            data_role_width: context.state(dp(190.0)),
            data_team_width: context.state(dp(160.0)),
            data_status_width: context.state(dp(180.0)),
            data_status: context.state("点击表头、拖动列边缘、重排列或右键单元格。".to_string()),
        }
    }

    fn view(&self) -> Element<Self> {
        let sidebar_app = self.clone();
        let content_app = self.clone();
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .style(styles::root_style)
            .child(
                Flex::horizontal()
                    .size(pct(100.0), pct(100.0))
                    .child(
                        self.current_page
                            .signal()
                            .map(move |page| navigation::sidebar(&sidebar_app, page)),
                    )
                    .child(
                        Flex::vertical().grow(1.0).height(pct(100.0)).child(
                            ScrollView::new().size(pct(100.0), pct(100.0)).child(
                                self.current_page
                                    .signal()
                                    .map(move |page| pages::render(&content_app, page)),
                            ),
                        ),
                    ),
            )
            .child(ToastHost::new(self.toast_queue.clone()).style(styles::modern_toast_style))
            .child(
                ToastHost::new(self.toast_top_start.clone())
                    .placement(ToastPlacement::TopStart)
                    .style(styles::modern_toast_style),
            )
            .child(
                ToastHost::new(self.toast_top_center.clone())
                    .placement(ToastPlacement::TopCenter)
                    .style(styles::modern_toast_style),
            )
            .child(
                ToastHost::new(self.toast_top_end.clone())
                    .placement(ToastPlacement::TopEnd)
                    .style(styles::modern_toast_style),
            )
            .child(
                ToastHost::new(self.toast_bottom_start.clone())
                    .placement(ToastPlacement::BottomStart)
                    .style(styles::modern_toast_style),
            )
            .child(
                ToastHost::new(self.toast_bottom_center.clone())
                    .placement(ToastPlacement::BottomCenter)
                    .style(styles::modern_toast_style),
            )
            .into()
    }
}

impl App {
    pub(crate) fn show_page(&mut self, page: DemoPage) {
        self.current_page.set(page);
    }

    pub(crate) fn code_expanded_signal(&self, id: &'static str) -> Signal<bool> {
        self.expanded_code
            .project(move |ids| ids.iter().any(|current| *current == id))
    }

    pub(crate) fn code_toggle_label(&self, id: &'static str) -> Signal<String> {
        self.code_expanded_signal(id).map(|expanded| {
            if expanded {
                "隐藏代码".to_string()
            } else {
                "显示代码".to_string()
            }
        })
    }

    pub(crate) fn toggle_code(&mut self, id: &'static str) {
        self.expanded_code.update(|ids| {
            if let Some(index) = ids.iter().position(|current| *current == id) {
                ids.remove(index);
            } else {
                ids.push(id);
            }
        });
    }

    pub(crate) fn load_audio_from_input(&mut self) {
        let source = self.input_text.text();
        let source = source.trim();
        if source.is_empty() {
            self.audio_status
                .set("请输入音频文件路径或 URL 后再加载。".to_string());
            return;
        }

        let audio_source = if source.starts_with("http://") || source.starts_with("https://") {
            AudioSource::url(source.to_string())
        } else {
            AudioSource::File(PathBuf::from(source))
        };

        match self.audio_controller.load(audio_source) {
            Ok(()) => self.audio_status.set(format!("已请求加载音频: {source}")),
            Err(error) => {
                let message = format!("加载音频失败: {error}");
                tgui_log(LogLevel::Error, &message);
                self.audio_status.set(message);
            }
        }
    }

    pub(crate) fn request_notification_permission(ctx: &CommandContext<Self>) {
        let _ =
            ctx.notifications()
                .request_permission(ValueCommand::new(|app: &mut App, result| {
                    app.notification_status.set(match result {
                        Ok(permission) => format!("通知权限: {permission:?}"),
                        Err(error) => format!("通知权限请求失败: {error}"),
                    });
                }));
    }

    pub(crate) fn send_plain_notification(&mut self, ctx: &CommandContext<Self>) {
        let result = ctx.notifications().send(
            NotificationOptions::new("TGUI Demo")
                .body("这是一条普通通知")
                .app_name("TGUI Demo"),
        );
        self.notification_status.set(match result {
            Ok(id) => format!("已发送普通通知: {id}"),
            Err(error) => {
                let message = format!("发送普通通知失败: {error}");
                tgui_log(LogLevel::Error, &message);
                message
            }
        });
    }

    pub(crate) fn send_action_notification(&mut self, ctx: &CommandContext<Self>) {
        let result = ctx.notifications().send_with_actions(
            NotificationOptions::new("TGUI Demo")
                .body("请选择一个动作，结果会回到 ViewModel。")
                .app_name("TGUI Demo")
                .action(NotificationAction::new("accept", "接受"))
                .action(NotificationAction::new("dismiss", "忽略")),
            ValueCommand::new(
                |app: &mut App, result: Result<NotificationActionEvent, NotificationError>| {
                    app.notification_status.set(match result {
                        Ok(event) => format!(
                            "通知动作: notification_id={}, action_id={}",
                            event.notification_id, event.action_id
                        ),
                        Err(error) => {
                            let message = format!("通知动作失败: {error}");
                            tgui_log(LogLevel::Error, &message);
                            message
                        }
                    });
                },
            ),
        );
        self.notification_status.set(match result {
            Ok(id) => format!("已发送动作通知: {id}"),
            Err(error) => {
                let message = format!("发送动作通知失败: {error}");
                tgui_log(LogLevel::Error, &message);
                message
            }
        });
    }

    pub(crate) fn toggle_left_drawer(&mut self) {
        self.drawer_left_open.update(|open| *open = !*open);
    }

    pub(crate) fn toggle_right_drawer(&mut self) {
        self.drawer_right_open.update(|open| *open = !*open);
    }

    pub(crate) fn toggle_top_drawer(&mut self) {
        self.drawer_top_open.update(|open| *open = !*open);
    }

    pub(crate) fn toggle_bottom_drawer(&mut self) {
        self.drawer_bottom_open.update(|open| *open = !*open);
    }

    pub(crate) fn toggle_push_drawer(&mut self) {
        self.drawer_push_open.update(|open| *open = !*open);
    }

    pub(crate) fn open_alert_modal(&mut self) {
        self.modal_alert_open.set(true);
    }

    pub(crate) fn dismiss_alert_modal(&mut self, _: bool) {
        self.modal_alert_open.set(false);
    }

    pub(crate) fn open_confirm_modal(&mut self) {
        self.modal_confirm_open.set(true);
    }

    pub(crate) fn dismiss_confirm_modal(&mut self, _: bool) {
        self.modal_confirm_open.set(false);
    }

    pub(crate) fn confirm_cancel(&mut self) {
        self.modal_confirm_result.set("已取消".to_string());
        self.modal_confirm_open.set(false);
    }

    pub(crate) fn confirm_yes(&mut self) {
        self.modal_confirm_result.set("已确认".to_string());
        self.modal_confirm_open.set(false);
    }

    pub(crate) fn open_form_modal(&mut self) {
        self.modal_form_open.set(true);
    }

    pub(crate) fn dismiss_form_modal(&mut self, _: bool) {
        self.modal_form_open.set(false);
    }

    pub(crate) fn submit_form_modal(&mut self) {
        let name = self.modal_form_name.text();
        let name = if name.trim().is_empty() {
            "未填写".to_string()
        } else {
            name
        };
        self.modal_form_result.set(format!("已提交: {name}"));
        self.modal_form_open.set(false);
    }

    pub(crate) fn set_list_selection(&mut self, change: ListSelectionChange) {
        let count = change.selected_keys.len();
        self.list_selected_keys.set(change.selected_keys);
        self.list_status.set(format!(
            "Selection via {:?}; focused={:?}; selected={count}",
            change.trigger, change.focused_key
        ));
    }

    pub(crate) fn open_list_item(&mut self, action: ListItemAction) {
        self.list_status.set(format!(
            "Primary action fired for row {} ({:?})",
            action.index, action.key
        ));
    }

    pub(crate) fn list_context_action(&mut self) {
        self.list_status
            .set("Context menu command selected for the current row".to_string());
    }

    pub(crate) fn clear_list_selection(&mut self) {
        self.list_selected_keys.set(Vec::new());
        self.list_status.set("Selection cleared".to_string());
    }

    pub(crate) fn toggle_list_loading(&mut self) {
        self.list_loading.update(|loading| *loading = !*loading);
        self.list_status.set("Loading slot toggled".to_string());
    }

    pub(crate) fn toggle_list_empty(&mut self) {
        self.list_show_empty.update(|empty| *empty = !*empty);
        self.list_status.set("Empty slot toggled".to_string());
    }

    pub(crate) fn select_data_rows(&mut self, change: DataGridSelectionChange) {
        let count = change.selected_keys.len();
        self.data_selected.set(change.selected_keys);
        self.data_status.set(format!(
            "Selection via {:?}; focused={:?}; selected={count}",
            change.trigger, change.focused_key
        ));
    }

    pub(crate) fn sort_data_rows(&mut self, change: DataGridSortChange) {
        let count = change.sort.len();
        self.data_sort.set(change.sort);
        self.data_status.set(format!(
            "Sort changed by {:?}; descriptors={count}",
            change.trigger
        ));
    }

    pub(crate) fn resize_data_column(&mut self, change: DataGridColumnWidthChange) {
        if change.column_key == WidgetKey::from("name") {
            self.data_name_width.set(change.width);
        } else if change.column_key == WidgetKey::from("role") {
            self.data_role_width.set(change.width);
        } else if change.column_key == WidgetKey::from("team") {
            self.data_team_width.set(change.width);
        } else if change.column_key == WidgetKey::from("status") {
            self.data_status_width.set(change.width);
        }
        self.data_status.set(format!(
            "Column {:?} resized to {}",
            change.column_key, change.width
        ));
    }

    pub(crate) fn reorder_data_column(&mut self, event: DataGridColumnReorderEvent) {
        self.data_column_order.update(|order| {
            if event.from_index >= order.len() || event.to_index >= order.len() {
                return;
            }
            let column = order.remove(event.from_index);
            order.insert(event.to_index, column);
        });
        self.data_status.set(format!(
            "Column {:?} moved before/onto {:?}",
            event.column_key, event.target_key
        ));
    }

    pub(crate) fn open_data_cell(&mut self, action: DataGridCellAction) {
        self.data_status.set(format!(
            "Cell action row={} column={:?}",
            action.row_index, action.column_key
        ));
    }

    pub(crate) fn commit_data_cell_edit(&mut self, commit: DataGridCellEditCommit) {
        self.data_rows.update(|rows| {
            if let Some(row) = rows
                .iter_mut()
                .find(|row| WidgetKey::from(row.id) == commit.row_key)
            {
                let value = format!("{}*", commit.value.trim_end_matches('*'));
                if commit.column_key == WidgetKey::from("name") {
                    row.name = value;
                } else if commit.column_key == WidgetKey::from("role") {
                    row.role = value;
                } else if commit.column_key == WidgetKey::from("status") {
                    row.status = value;
                }
            }
        });
        self.data_status.set(format!(
            "Committed edit for row {:?}, column {:?}",
            commit.row_key, commit.column_key
        ));
    }

    pub(crate) fn mark_data_reviewed(&mut self) {
        let selected = self.data_selected.get();
        self.data_rows.update(|rows| {
            for row in rows {
                if selected.iter().any(|key| key == &WidgetKey::from(row.id)) {
                    row.status = "Reviewed".to_string();
                }
            }
        });
        self.data_status
            .set("Marked selected rows as reviewed".to_string());
    }

    pub(crate) fn clear_data_selection(&mut self) {
        self.data_selected.set(Vec::new());
        self.data_status.set("Selection cleared".to_string());
    }

    fn theme_binding(&self) -> Signal<ThemeMode> {
        self.theme.signal()
    }

    fn reduced_motion_binding(&self) -> Signal<bool> {
        self.reduced_motion.signal()
    }

    pub(crate) fn run() -> Result<(), TguiError> {
        Application::new()
            .app_id("com.tgui.demo")
            .title("TGUI Component Demo")
            .window_size(dp(1180.0), dp(760.0))
            .with_view_model(App::new)
            .root_view(App::view)
            .bind_theme_mode(App::theme_binding)
            .bind_reduced_motion(App::reduced_motion_binding)
            .run()
    }
}

pub(crate) fn audio_status_text(state: AudioPlaybackState) -> String {
    match state {
        AudioPlaybackState::Idle => "播放状态: 等待".to_string(),
        AudioPlaybackState::Loading => "播放状态: 加载中".to_string(),
        AudioPlaybackState::Ready => "播放状态: 准备".to_string(),
        AudioPlaybackState::Playing => "播放状态: 播放中".to_string(),
        AudioPlaybackState::Paused => "播放状态: 暂停中".to_string(),
        AudioPlaybackState::Buffering => "播放状态: 缓冲中".to_string(),
        AudioPlaybackState::Ended => "播放状态: 播放结束".to_string(),
        AudioPlaybackState::Error(error) => format!("播放状态: 出错: {error}"),
    }
}

fn video_status_text(state: VideoPlaybackState) -> String {
    match state {
        VideoPlaybackState::Idle => "视频状态: 等待".to_string(),
        VideoPlaybackState::Loading => "视频状态: 加载中".to_string(),
        VideoPlaybackState::Ready => "视频状态: 准备".to_string(),
        VideoPlaybackState::Playing => "视频状态: 播放中".to_string(),
        VideoPlaybackState::Paused => "视频状态: 暂停中".to_string(),
        VideoPlaybackState::Buffering => "视频状态: 缓冲中".to_string(),
        VideoPlaybackState::Ended => "视频状态: 播放结束".to_string(),
        VideoPlaybackState::Error(error) => format!("视频状态: 出错: {error}"),
    }
}
