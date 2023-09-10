use std::collections::HashSet;
use std::ffi::{c_char, CStr, CString};
use std::mem::ManuallyDrop;

use anyhow::Context;
use ash::{Entry, Instance, vk};
use ash::vk::ApplicationInfo;
use ash_window::enumerate_required_extensions;
use glfw::{Action, Glfw, Key, Window, WindowEvent, WindowHint, WindowMode};
use glfw::ClientApiHint::NoApi;
use raw_window_handle::HasRawDisplayHandle;
use vk::{API_VERSION_1_2, InstanceCreateInfo};

// Vk context object
// uses ManuallyDrop to control drop order
pub struct Vk {
    entry: ManuallyDrop<Entry>,
    instance: ManuallyDrop<Instance>,
}

impl Vk {
    fn new(display_handle: &dyn HasRawDisplayHandle) -> anyhow::Result<Self> {
        let entry = create_entry()?;
        let instance = create_instance(&entry, display_handle)?;
        Ok(Self {
            entry: ManuallyDrop::new(entry),
            instance: ManuallyDrop::new(instance),
        })
    }
}

impl Drop for Vk {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.instance);
            ManuallyDrop::drop(&mut self.entry);
        }
    }
}

pub struct RuntimeContext {
    glfw: Glfw,
    main_window: Window,
    vk: Vk,
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

    let vk = Vk::new(&main_window)?;
    let mut ctx = RuntimeContext { glfw, main_window, vk };

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

fn create_entry() -> anyhow::Result<Entry> {
    Ok(Entry::linked())
}

fn create_instance(entry: &Entry, display_handle: &dyn HasRawDisplayHandle) -> anyhow::Result<Instance> {
    let mut required_extensions: Vec<_> = enumerate_required_extensions(display_handle.raw_display_handle())?
        .iter()
        .map(|e| unsafe { CString::from(CStr::from_ptr(*e)) })
        .collect();

    let mut instance_create_flags = vk::InstanceCreateFlags::empty();
    // required by MoltenVK
    #[cfg(target_os = "macos")]
    {
        required_extensions
            .push(CString::new("VK_KHR_portability_enumeration").unwrap());
        instance_create_flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
    }

    let required_extensions_ptr: Vec<_> =
        required_extensions.iter().map(|arg| arg.as_ptr()).collect();

    let layers = if cfg!(feature = "validation_layers") {
        let required_layers = HashSet::from(["VK_LAYER_KHRONOS_validation"]);

        let res: Vec<_> = entry
            .enumerate_instance_layer_properties()
            .expect(
                "validation layers should be available when `validation_layers` feature is enabled.",
            )
            .iter()
            .map(|layer_info| layer_info.layer_name)
            .filter(|e| unsafe {
                let c_str = CStr::from_ptr(e.as_ptr());
                required_layers.contains(c_str.to_str().unwrap())
            })
            .collect();

        if required_layers.len() != res.len() {
            panic!("required layers not found");
        }

        res
    } else {
        vec![]
    };

    let layers_ptr = layers
        .iter()
        .map(|l| l.as_ptr())
        .collect::<Vec<*const c_char>>();

    let create_info = InstanceCreateInfo::builder()
        .enabled_extension_names(required_extensions_ptr.as_slice())
        .enabled_layer_names(layers_ptr.as_slice())
        .flags(instance_create_flags)
        .application_info(&ApplicationInfo::builder().api_version(API_VERSION_1_2).build())
        .build();

    unsafe {
        entry.create_instance(&create_info, None).context("failed to create instance")
    }
}