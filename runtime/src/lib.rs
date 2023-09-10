use anyhow::Context;
use glfw::{Action, Glfw, Key, Window, WindowEvent, WindowHint, WindowMode};
use glfw::ClientApiHint::NoApi;

pub struct RuntimeContext {
    glfw: Glfw,
    main_window: Window,
}

impl RuntimeContext {
    pub fn glfw(&self) -> &Glfw {
        &self.glfw
    }

    pub fn main_window(&self) -> &Window {
        &self.main_window
    }
}

pub trait App {
    fn should_auto_close(&self) -> bool {
        true
    }

    fn get_title(&mut self) -> anyhow::Result<String>;

    fn init(&mut self, ctx: &mut RuntimeContext) -> anyhow::Result<()> {
        Ok(())
    }

    fn event(&mut self, ctx: &mut RuntimeContext, event: WindowEvent) -> anyhow::Result<()> {
        Ok(())
    }


    fn frame(&mut self, ctx: &mut RuntimeContext) -> anyhow::Result<()>;
}

pub fn run(mut app: impl App) -> anyhow::Result<()> {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)?;
    glfw.window_hint(WindowHint::ClientApi(NoApi));
    let (mut main_window, events) = glfw.create_window(1920, 1080, &app.get_title()?, WindowMode::Windowed).context("failed to create main window")?;
    main_window.set_key_polling(true);
    let mut ctx = RuntimeContext { glfw, main_window };

    while !ctx.main_window.should_close() {
        app.frame(&mut ctx)?;
        ctx.glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            if app.should_auto_close() {
                if let WindowEvent::Key(Key::Escape, _, Action::Press, _) = event {
                    ctx.main_window.set_should_close(true);
                    break;
                }
            }
            app.event(&mut ctx, event.clone())?;
        }
    }
    Ok(())
}