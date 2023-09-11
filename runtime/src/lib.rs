use std::ffi::{CStr, CString};
use std::mem::ManuallyDrop;

use anyhow::Context;
use ash::{Device, Entry, Instance};
use ash::vk::{PhysicalDevice, Queue};
use glfw::{Action, Glfw, Key, Window, WindowEvent, WindowHint, WindowMode};
use glfw::ClientApiHint::NoApi;
use raw_window_handle::HasRawDisplayHandle;

use crate::vk_utils::{create_device, create_entry, create_instance, find_queue_family_indices, select_physical_device};

mod vk_utils;

// Vk context object
// uses ManuallyDrop to control drop order
pub struct Vk {
    entry: ManuallyDrop<Entry>,
    instance: ManuallyDrop<Instance>,
    physical_device: ManuallyDrop<PhysicalDevice>,
    queue_family_idx: u32,
    device: ManuallyDrop<Device>,
    queue: ManuallyDrop<Queue>,
}

impl Vk {
    fn new(display_handle: &dyn HasRawDisplayHandle) -> anyhow::Result<Self> {
        let entry = create_entry()?;
        let instance = create_instance(&entry, display_handle)?;
        let required_device_extensions = get_required_device_extensions();
        let physical_device = select_physical_device(&instance, &required_device_extensions)?;
        let queue_family_idx = find_queue_family_indices(&instance, physical_device);
        let device = create_device(&instance, physical_device, queue_family_idx, &required_device_extensions)?;
        let queue = unsafe {
            device.get_device_queue(queue_family_idx, 0)
        };
        Ok(Self {
            entry: ManuallyDrop::new(entry),
            instance: ManuallyDrop::new(instance),
            physical_device: ManuallyDrop::new(physical_device),
            queue_family_idx,
            device: ManuallyDrop::new(device),
            queue: ManuallyDrop::new(queue),
        })
    }

    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    pub fn physical_device(&self) -> &PhysicalDevice {
        &self.physical_device
    }

    pub fn queue_family_idx(&self) -> u32 {
        self.queue_family_idx
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }
}

impl Drop for Vk {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self.physical_device);
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

fn get_required_device_extensions() -> Vec<CString> {
    vec![
        // required by MoltenVK
        unsafe { CStr::from_ptr("VK_KHR_portability_subset\0".as_ptr().cast()) },
        ash::extensions::khr::Swapchain::name(),
        ash::extensions::khr::DynamicRendering::name(),
    ]
        .into_iter()
        .map(|e| CString::from(e))
        .collect()
}