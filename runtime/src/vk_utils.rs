use std::collections::{BTreeMap, HashSet};
use std::ffi::{c_char, CStr, CString};

use anyhow::{bail, Context};
use ash::{Device, Entry, Instance, vk};
use ash::vk::{API_VERSION_1_2, ApplicationInfo, InstanceCreateInfo, SurfaceKHR};
use ash_window::enumerate_required_extensions;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use vk::{DeviceCreateInfo, DeviceQueueCreateInfo, PhysicalDevice, PhysicalDeviceDynamicRenderingFeaturesKHR, PhysicalDeviceFeatures, PhysicalDeviceType};

pub fn create_entry() -> anyhow::Result<Entry> {
    Ok(Entry::linked())
}

pub fn create_instance(entry: &Entry, display_handle: &dyn HasRawDisplayHandle) -> anyhow::Result<Instance> {
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

pub fn select_physical_device(
    instance: &Instance,
    required_device_extensions: &Vec<CString>,
) -> anyhow::Result<PhysicalDevice> {
    let physical_devices = unsafe {
        instance
            .enumerate_physical_devices()
            .expect("physical devices should be available.")
    };
    let mut candidates = BTreeMap::<u32, PhysicalDevice>::new();
    for physical_device in physical_devices {
        let mut score: u32 = 0;
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };
        let _features = unsafe { instance.get_physical_device_features(physical_device) };

        // bias towards discrete gpus
        score += match properties.device_type {
            PhysicalDeviceType::DISCRETE_GPU => 1000,
            PhysicalDeviceType::INTEGRATED_GPU => 100,
            _ => 0,
        };
        // prefer device that support larger image dimensions
        score += properties.limits.max_image_dimension2_d;

        candidates.insert(score, physical_device);
    }

    let physical_device = candidates
        .last_entry()
        .context("suitable device should be found.")?
        .remove();

    let actual_device_extensions: HashSet<String> = unsafe {
        instance
            .enumerate_device_extension_properties(physical_device)
            .context("physical device extensions should be enumerable.")?
            .iter()
            .map(|e| {
                CStr::from_ptr(e.extension_name.as_ptr())
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    };

    let mut count = 0;
    for required_extension in required_device_extensions.iter() {
        let key = required_extension.to_str().unwrap().to_string();
        if actual_device_extensions.contains(&key) {
            count += 1;
        }
    }
    if count != required_device_extensions.len() {
        bail!("device is missing required extensions")
    }

    Ok(physical_device)
}

pub fn find_queue_family_indices(
    instance: &Instance,
    physical_device: PhysicalDevice,
) -> u32 {
    let queue_families =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
    for (index, queue_family) in queue_families.into_iter().enumerate() {
        if queue_family
            .queue_flags
            .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE) // assume present is supported
        {
            return index as u32;
        }
    }

    panic!("failed to find queue family that supports GRAPHICS, COMPUTE and PRESENT")
}

pub fn create_device(
    instance: &Instance,
    physical_device: PhysicalDevice,
    queue_family_idx: u32,
    required_device_extensions: &Vec<CString>,
) -> anyhow::Result<Device> {
    let queue_create_infos = [DeviceQueueCreateInfo::builder()
        .queue_family_index(queue_family_idx)
        .queue_priorities(&[1.0])
        .build()];

    let physical_device_features = PhysicalDeviceFeatures::default();
    // enable dynamic rendering
    let mut dynamic_rendering = PhysicalDeviceDynamicRenderingFeaturesKHR::builder()
        .dynamic_rendering(true)
        .build();

    let required_device_extensions_ptr: Vec<_> = required_device_extensions
        .iter()
        .map(|e| e.as_c_str().as_ptr())
        .collect();
    let device_create_info = DeviceCreateInfo::builder()
        .queue_create_infos(&queue_create_infos)
        .enabled_features(&physical_device_features)
        .enabled_extension_names(required_device_extensions_ptr.as_slice())
        .push_next(&mut dynamic_rendering)
        .build();
    unsafe {
        Ok(instance
            .create_device(physical_device, &device_create_info, None)
            .expect("create_device successful."))
    }
}

pub fn create_surface(
    entry: &Entry,
    instance: &Instance,
    window: &(impl HasRawDisplayHandle + HasRawWindowHandle),
) -> anyhow::Result<SurfaceKHR> {
    let vk_surface = unsafe {
        ash_window::create_surface(
            entry,
            instance,
            window.raw_display_handle(),
            window.raw_window_handle(),
            None,
        )?
    };
    Ok(vk_surface)
}