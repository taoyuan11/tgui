use crate::gui::core::Canvas;
use crate::gui::style::Color;
use crate::gui::widgets::{GuiEvent, Panel, Widget};
use softbuffer::{Context, Surface};
use std::cell::RefCell;
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::dpi::{LogicalSize, LogicalPosition};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

// 用于延迟执行的回调队列
thread_local! {
    static PENDING_CALLBACKS: RefCell<Vec<Box<dyn FnOnce()>>> = RefCell::new(Vec::new());
}

pub mod gui;

// 全局单例 ApplicationWindow（使用线程局部存储）
thread_local! {
    static APP_WINDOW: RefCell<Option<ApplicationWindow>> = RefCell::new(None);
}

/// 访问全局 ApplicationWindow 实例
pub fn window<F, R>(f: F) -> R
where
    F: FnOnce(&mut ApplicationWindow) -> R,
{
    APP_WINDOW.with(|app| {
        let mut app_ref = app.borrow_mut();
        let app_window = app_ref.as_mut().expect(
            "ApplicationWindow not initialized. Call WindowBuilder::new().run() first."
        );

        f(app_window)
    })
}

/// 将回调加入延迟执行队列，在事件处理完成后执行
pub fn defer<F>(callback: F)
where
    F: FnOnce() + 'static,
{
    PENDING_CALLBACKS.with(|callbacks| {
        callbacks.borrow_mut().push(Box::new(callback));
    });
}

/// 执行所有待处理的回调
fn execute_pending_callbacks() {
    PENDING_CALLBACKS.with(|callbacks| {
        let mut cbs = callbacks.borrow_mut();
        let pending: Vec<_> = cbs.drain(..).collect();
        drop(cbs); // 释放借用
        
        for callback in pending {
            callback();
        }
    });
}

pub struct ApplicationWindow {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    root: Panel,
    cursor: (f64, f64),
    window_title: String,
    background_color: Color,
    window_size: (u32, u32),
    window_position: (i32, i32),
}

/// 窗口构建器，用于创建和配置窗口
pub struct WindowBuilder {}

impl ApplicationHandler for WindowBuilder {
    fn resumed(&mut self, el: &ActiveEventLoop) {

        window(|app| {
            let mut attrs = Window::default_attributes()
                .with_title(&app.window_title)
                .with_inner_size(LogicalSize::new(app.window_size.0, app.window_size.1));

            attrs = attrs.with_position(LogicalPosition::new(app.window_position.0, app.window_position.1));


            let win = Rc::new(el.create_window(attrs).unwrap());
            let ctx = Context::new(win.clone()).unwrap();
            let surf = Surface::new(&ctx, win.clone()).unwrap();

            app.window = Some(win);
            app.surface = Some(surf);
        });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        window(|app| {
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    app.cursor = (position.x, position.y);
                    let gui_evt = GuiEvent::MouseMove { x: position.x, y: position.y };
                    app.root.handle_event(&gui_evt, 0, 0);
                }
                WindowEvent::MouseInput { state, button, .. } => {

                    let gui_evt = match state {
                        ElementState::Pressed => GuiEvent::MouseDown { x: app.cursor.0, y: app.cursor.1, button },
                        ElementState::Released => GuiEvent::MouseUp { x: app.cursor.0, y: app.cursor.1, button },
                    };

                    if app.root.handle_event(&gui_evt, 0, 0) {
                        if let Some(window) = &app.window {
                            window.request_redraw()
                        }
                    }
                }
                WindowEvent::KeyboardInput { event, ..} => {
                    let keycode = if let PhysicalKey::Code(keycode) = event.physical_key {
                        keycode
                    } else {  return };

                    let gui_evt = match event.state {
                        ElementState::Pressed => {
                            GuiEvent::KeyDown { key: keycode }
                        }
                        ElementState::Released => {
                            GuiEvent::KeyUp { key: keycode }
                        }
                    };
                    if app.root.handle_event(&gui_evt, 0, 0) {
                        if let Some(window) = &app.window {
                            window.request_redraw()
                        }
                    }
                }
                // ... 其他逻辑
                WindowEvent::RedrawRequested => {
                    if let Some(surface) = &mut app.surface {
                        if let Some(window) = &mut app.window {
                            let size = window.inner_size();

                            let width = size.width;
                            let height = size.height;

                            // 只有当窗口大小大于 0 时才进行渲染（防止最小化时崩溃）
                            if width > 0 && height > 0 {
                                // 必须调用 resize，否则 buffer_mut 会报错
                                surface.resize(
                                    std::num::NonZeroU32::new(width).unwrap(),
                                    std::num::NonZeroU32::new(height).unwrap(),
                                ).unwrap();

                                let mut buffer = surface.buffer_mut().unwrap();

                                // 创建画布
                                let mut canvas = Canvas::new(&mut buffer, width, height);

                                canvas.fill(app.background_color);

                                app.root.draw(&mut canvas, 0, 0);

                                buffer.present().unwrap();
                            }
                        }
                    }
                }
                WindowEvent::CloseRequested => el.exit(),
                _ => (),
            }
        });
        
        // 在释放 APP_WINDOW 的借用后，执行所有待处理的回调
        execute_pending_callbacks();
    }
}

impl ApplicationWindow {
    pub fn set_title(&mut self, title: &str) {
        self.window_title = title.to_string();
        if let Some(window) = &self.window {
            window.set_title(&self.window_title)
        }
    }

    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.root.add_child(child);
    }

    pub fn get_window(&self) -> Option<&Rc<Window>> {
        self.window.as_ref()
    }
}

impl WindowBuilder {
    /// 创建新的窗口构建器
    pub fn new(title: &str) {

        let app = ApplicationWindow {
            window: None,
            surface: None,
            root: Panel::new(0, 0),
            cursor: (0.0, 0.0),
            window_title: title.to_string(),
            background_color: Color::BLACK,
            window_size: (800, 600),
            window_position: (500, 550),
        };

        // 初始化全局单例
        APP_WINDOW.with(|global| {
            let mut global_ref = global.borrow_mut();
            if global_ref.is_some() {
                panic!("ApplicationWindow already initialized");
            }
            *global_ref = Some(app);
        });
    }


    /// 运行窗口，消费 WindowBuilder 所有权并初始化全局单例
    pub fn run() {
        
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(ControlFlow::Wait);

        event_loop.run_app(&mut WindowBuilder{}).unwrap();
    }
}