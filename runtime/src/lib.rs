use std::ffi::{CStr, CString};
use std::mem::ManuallyDrop;
use std::ops::Deref;

use anyhow::Context;
use ash::vk::{
    ColorSpaceKHR, ComponentMapping, CompositeAlphaFlagsKHR, Extent2D, Format, Image,
    ImageAspectFlags, ImageSubresourceRange, ImageUsageFlags, ImageView, ImageViewCreateInfo,
    ImageViewType, PhysicalDevice, PresentModeKHR, Queue, SurfaceKHR, SurfaceTransformFlagsKHR,
    SwapchainCreateInfoKHR, SwapchainKHR,
};
use ash::{Device, Entry, Instance};
use glfw::ClientApiHint::NoApi;
use glfw::{Action, Glfw, Key, Window, WindowEvent, WindowHint, WindowMode};
use raw_window_handle::HasRawDisplayHandle;

use crate::vk_utils::{
    create_device, create_entry, create_instance, create_surface, find_queue_family_indices,
    select_physical_device,
};

mod vk_utils;

struct SwapchainHolder {
    swapchain: SwapchainKHR,
    images: Vec<Image>,
    image_views: Vec<ImageView>,
}

impl SwapchainHolder {
    fn destroy(self, vk: &Vk) {
        unsafe {
            for image_view in self.image_views {
                vk.device().destroy_image_view(image_view, None)
            }

            vk.khr_swapchain().destroy_swapchain(self.swapchain, None);
        }
    }
}

// Vk context object
// uses ManuallyDrop to control drop order
pub struct Vk {
    entry: ManuallyDrop<Entry>,
    khr_surface: ManuallyDrop<ash::extensions::khr::Surface>,
    khr_swapchain: ManuallyDrop<ash::extensions::khr::Swapchain>,
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
        let device = create_device(
            &instance,
            physical_device,
            queue_family_idx,
            &required_device_extensions,
        )?;
        let khr_surface = ash::extensions::khr::Surface::new(&entry, &instance);
        let khr_swapchain = ash::extensions::khr::Swapchain::new(&instance, &device);
        let queue = unsafe { device.get_device_queue(queue_family_idx, 0) };
        Ok(Self {
            entry: ManuallyDrop::new(entry),
            khr_surface: ManuallyDrop::new(khr_surface),
            khr_swapchain: ManuallyDrop::new(khr_swapchain),
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

    pub fn khr_surface(&self) -> &ash::extensions::khr::Surface {
        &self.khr_surface
    }

    pub fn khr_swapchain(&self) -> &ash::extensions::khr::Swapchain {
        &self.khr_swapchain
    }

    pub fn instance(&self) -> &Instance {
        &self.instance
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
            self.device.destroy_device(None);
            ManuallyDrop::drop(&mut self.physical_device);
            self.instance.destroy_instance(None);
            ManuallyDrop::drop(&mut self.entry);
        }
    }
}

pub struct AppContext {
    glfw: Glfw,
    main_window: Window,
    main_surface: SurfaceKHR,
    vk: Vk,
    swapchain: Option<SwapchainHolder>,
}

impl AppContext {
    pub fn glfw(&self) -> &Glfw {
        &self.glfw
    }

    pub fn main_window(&self) -> &Window {
        &self.main_window
    }

    fn recreate_swapchain(&mut self, app: &impl App) -> anyhow::Result<()> {
        if let Some(old_swapchain) = self.swapchain.take() {
            old_swapchain.destroy(&self.vk);
        }

        let (width, height) = self.main_window.get_framebuffer_size();
        let swapchain = create_swapchain(
            &self.vk,
            &self.main_surface,
            app.get_swapchain_format()?,
            app.get_swapchain_color_space()?,
            ImageUsageFlags::COLOR_ATTACHMENT,
            Extent2D::builder()
                .width(width as u32)
                .height(height as u32)
                .build(),
            app.get_swapchain_min_image_count()?,
        )?;

        self.swapchain = Some(swapchain);

        Ok(())
    }
}

impl Drop for AppContext {
    fn drop(&mut self) {
        unsafe {
            if let Some(swapchain) = self.swapchain.take() {
                swapchain.destroy(&self.vk);
            }
            self.vk.khr_surface.destroy_surface(self.main_surface, None);
        }
    }
}

pub trait App {
    fn should_auto_close(&self) -> bool {
        true
    }

    fn get_swapchain_min_image_count(&self) -> anyhow::Result<u32> {
        Ok(3)
    }

    fn get_swapchain_format(&self) -> anyhow::Result<Format> {
        Ok(Format::B8G8R8A8_SRGB)
    }

    fn get_swapchain_color_space(&self) -> anyhow::Result<ColorSpaceKHR> {
        Ok(ColorSpaceKHR::SRGB_NONLINEAR)
    }

    fn get_title(&mut self) -> anyhow::Result<String>;

    fn init(&mut self, ctx: &mut AppContext) -> anyhow::Result<()> {
        Ok(())
    }

    fn event(&mut self, ctx: &mut AppContext, event: WindowEvent) -> anyhow::Result<()> {
        Ok(())
    }

    fn frame(&mut self, ctx: &mut AppContext) -> anyhow::Result<()>;
}

pub fn run(mut app: impl App) -> anyhow::Result<()> {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)?;
    glfw.window_hint(WindowHint::ClientApi(NoApi));
    let (mut main_window, events) = glfw
        .create_window(1920, 1080, &app.get_title()?, WindowMode::Windowed)
        .context("failed to create main window")?;
    main_window.set_key_polling(true);

    let vk = Vk::new(&main_window)?;
    let main_surface = create_surface(vk.entry(), vk.instance(), &main_window)?;
    let mut ctx = AppContext {
        glfw,
        main_window,
        main_surface,
        vk,
        swapchain: None,
    };

    ctx.recreate_swapchain(&app)?;

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

            if let WindowEvent::FramebufferSize(_, _) = event {
                ctx.recreate_swapchain(&app)?;
                continue;
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

fn create_swapchain(
    vk: &Vk,
    surface: &SurfaceKHR,
    image_format: Format,
    image_color_space: ColorSpaceKHR,
    image_usage: ImageUsageFlags,
    image_extent: Extent2D,
    min_image_count: u32,
) -> anyhow::Result<SwapchainHolder> {
    let create_info = SwapchainCreateInfoKHR::builder()
        .surface(surface.clone())
        .image_format(image_format)
        .image_usage(image_usage)
        .image_extent(image_extent)
        .present_mode(PresentModeKHR::FIFO)
        .pre_transform(SurfaceTransformFlagsKHR::IDENTITY)
        .image_array_layers(1)
        .min_image_count(min_image_count)
        .clipped(true)
        .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
        .image_color_space(image_color_space)
        .build();

    let swapchain = unsafe {
        vk.khr_swapchain()
            .create_swapchain(&create_info, None)
            .context("failed to create swapchain")?
    };

    let images = unsafe { vk.khr_swapchain().get_swapchain_images(swapchain)? };

    let mut image_views = vec![];

    for image in &images {
        let create_info = ImageViewCreateInfo::builder()
            .format(image_format)
            .view_type(ImageViewType::TYPE_2D)
            .image(image.clone())
            .components(ComponentMapping::builder().build())
            .subresource_range(
                ImageSubresourceRange::builder()
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .level_count(1)
                    .build(),
            )
            .build();

        let image_view = unsafe {
            vk.device
                .create_image_view(&create_info, None)
                .context("failed to create image view")?
        };

        image_views.push(image_view);
    }
    Ok(SwapchainHolder {
        swapchain,
        images,
        image_views,
    })
}
