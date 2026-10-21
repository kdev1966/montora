// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! The one GPU device the window and the engine share.
//!
//! Slint renders the window on it, and the monitor composites on it. Because
//! both sides hold the same device, a composited frame is a texture the
//! renderer samples directly: nothing is read back and nothing is copied.
//! Created before the backend is selected, because Slint takes the device at
//! selection time; `None` when the machine offers no adapter, and then the
//! window renders the way it did without one.
//!
//! On Android the backend owns its device and none is shared, so the type
//! is carried and never opened there; see platform.rs.
#![cfg_attr(target_os = "android", allow(dead_code))]

use std::sync::OnceLock;

/// The adapter this run opened, as one line for Settings > About: name,
/// backend, and whether it is a software rasteriser. Written once by
/// [`Gpu::acquire`], because the adapter itself is moved into the monitor
/// before the About sheet is built.
static ADAPTER: OnceLock<String> = OnceLock::new();

/// What [`Gpu::acquire`] found, or `None` when it found nothing.
pub fn adapter_description() -> Option<&'static str> {
    ADAPTER.get().map(String::as_str)
}

/// Which GPU to ask for: the most powerful, unless `MONTORA_GPU=integrated`
/// asks for the machine's integrated one. On a Mac with two GPUs the
/// discrete one is not always the one that holds up: a 2015 MacBook Pro's
/// Radeon R9 M370X stalled on the effect cards' shaders under macOS 12
/// until the kernel restarted it, taking the session down with it. This
/// lets such a machine run on its other GPU. Read in both places an
/// adapter is chosen: here and in concat-render's gpu.rs.
pub fn power_preference() -> wgpu::PowerPreference {
    match std::env::var("MONTORA_GPU").as_deref().map(str::trim) {
        Ok("integrated" | "low-power") => wgpu::PowerPreference::LowPower,
        _ => wgpu::PowerPreference::HighPerformance,
    }
}

/// The GPU choices Settings > General offers, in its order, as the names
/// the settings file and `MONTORA_GPU` use.
pub const CHOICES: [&str; 3] = ["auto", "integrated", "discrete"];

/// Applies the remembered GPU choice for this run by setting `MONTORA_GPU`,
/// which both places an adapter is chosen read - unless it is already set,
/// so a value given by hand still wins. "auto", or nothing remembered,
/// asks for the integrated GPU only where the discrete one is known to
/// fail: see [`integrated_is_safer`].
///
/// # Safety
///
/// Writes the environment: call it while the process has one thread.
pub unsafe fn apply_choice(choice: Option<&str>) {
    if std::env::var_os("MONTORA_GPU").is_some() {
        return;
    }
    let integrated = match choice {
        Some("integrated") => true,
        Some("discrete") => false,
        _ => integrated_is_safer(),
    };
    if integrated {
        log::info!("gpu: asking for the integrated GPU");
        // SAFETY: the caller promises a single-threaded process.
        unsafe { std::env::set_var("MONTORA_GPU", "integrated") };
    }
}

/// Whether this machine is one whose discrete GPU stalls on the engine's
/// shaders while its integrated one does not: a Mac on macOS 12 or older
/// with an AMD GPU beside an integrated one. The 2015 MacBook Pro's Radeon
/// R9 M370X hung on the effect cards until the kernel restarted it and took
/// the session down; the Intel Iris Pro beside it draws them.
fn integrated_is_safer() -> bool {
    if concat_host::machine::macos_major().is_none_or(|major| major > 12) {
        return false;
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapters: Vec<wgpu::AdapterInfo> =
        pollster::block_on(instance.enumerate_adapters(wgpu::Backends::METAL))
            .iter()
            .map(wgpu::Adapter::get_info)
            .collect();
    amd_beside_integrated(&adapters)
}

/// Whether `adapters` hold an AMD discrete GPU and an integrated one.
/// Metal gives no PCI ids (wgpu reports vendor 0 there), so AMD is also
/// known by its name: "AMD Radeon R9 M370X".
fn amd_beside_integrated(adapters: &[wgpu::AdapterInfo]) -> bool {
    let integrated = adapters
        .iter()
        .any(|info| info.device_type == wgpu::DeviceType::IntegratedGpu);
    let amd = adapters.iter().any(|info| {
        let name = info.name.to_ascii_lowercase();
        // 0x1002 is AMD's PCI vendor id.
        info.device_type == wgpu::DeviceType::DiscreteGpu
            && (info.vendor == 0x1002 || name.contains("amd") || name.contains("radeon"))
    });
    integrated && amd
}

/// The shared device and what it was created from.
#[derive(Clone)]
pub struct Gpu {
    /// The instance, which Slint needs to make a surface for the window.
    pub instance: wgpu::Instance,
    /// The adapter, which Slint reads the backend off.
    pub adapter: wgpu::Adapter,
    /// The device both sides draw on.
    pub device: wgpu::Device,
    /// The one queue, so submissions from both sides are ordered.
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// Opens the machine's best adapter and a device on it.
    pub fn acquire() -> Option<Gpu> {
        // Only the backend Slint's Skia renderer can share a device on:
        // Skia is built against the platform's own API, so a device on any
        // other backend is refused at selection time. PRIMARY let wgpu pick
        // Vulkan on Windows machines that offer it, and the window then
        // failed to open with "Unsupported WGPU backend for use with Skia".
        let backends = if cfg!(target_vendor = "apple") {
            wgpu::Backends::METAL
        } else if cfg!(windows) {
            wgpu::Backends::DX12
        } else {
            wgpu::Backends::VULKAN
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            // Without wgpu's validation of indirect calls: it compiles a
            // compute shader of its own the moment a device opens, and the
            // Metal compiler of an Intel Iris Pro under macOS 12 fails on it
            // ("Compiler encountered an internal error"), so no device opened
            // on that GPU at all. Nothing here draws or dispatches
            // indirectly, so there is nothing for it to check.
            flags: wgpu::InstanceFlags::from_build_config()
                .difference(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: power_preference(),
            ..Default::default()
        }))
        .ok()?;
        // This is not always a GPU. wgpu ranks a CPU adapter last but never
        // leaves it out, so on a machine whose hardware it will not drive -
        // its D3D12 backend wants resource binding tier 2, which Intel's
        // Broadwell-era chips do not have - the adapter that comes back is
        // WARP, Windows' software rasteriser. The window still opens on it;
        // see `is_software` and platform.rs for what that changes.
        // https://github.com/jub0t/Concat/issues/135
        let info = adapter.get_info();
        log::info!(
            "GPU adapter: {} ({:?}, {:?}, {})",
            info.name,
            info.device_type,
            info.backend,
            driver_of(&info)
        );
        let _ = ADAPTER.set(if info.device_type == wgpu::DeviceType::Cpu {
            format!("{} · {} · software rasteriser", info.name, info.backend)
        } else {
            format!("{} · {}", info.name, info.backend)
        });
        // The adapter's own limits, not `Limits::default()`. The defaults are
        // WebGPU's baseline, which a Vulkan, Metal or Direct3D 12 GPU meets
        // and an OpenGL ES one may not - asking for them opened no device
        // on such phones (#312) - and their `max_texture_dimension_2d` is
        // 8192 whatever the machine can do: 16384 on any current desktop
        // GPU. What the adapter reports it can always open, and both sides
        // of this device want the real number: the monitor composites the
        // timeline's frame, and Slint's renderer allocates a layer as large
        // as the element it is flattening.
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("concat"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .ok()?;
        // Said in the log when it dies, with wgpu's reason: the one line a
        // report of "the GPU device was lost" needs, which until now there
        // was none of. https://github.com/jub0t/Concat/issues/223
        device.set_device_lost_callback(|reason, message| {
            log::error!("the window's GPU device was lost ({reason:?}): {message}");
        });
        // An error no scope caught goes to the log too. wgpu's default
        // handler panics, and on this device that is the window's thread:
        // a texture past the limit, asked for by anything that draws on it,
        // took the app down where a bad frame would do.
        device.on_uncaptured_error(std::sync::Arc::new(|error: wgpu::Error| {
            log::error!("the window's GPU device reported an error: {error}");
        }));
        Some(Gpu {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Whether the adapter is a software rasteriser - WARP on Windows,
    /// llvmpipe on Linux - rather than a GPU. Everything still draws, only
    /// slower, and the sysinfo sheet says so.
    pub fn is_software(&self) -> bool {
        self.adapter.get_info().device_type == wgpu::DeviceType::Cpu
    }

    /// The device as Slint takes it.
    pub fn configuration(&self) -> slint::wgpu_30::WGPUConfiguration {
        slint::wgpu_30::WGPUConfiguration::Manual {
            instance: self.instance.clone(),
            adapter: self.adapter.clone(),
            device: self.device.clone(),
            queue: self.queue.clone(),
        }
    }
}

/// The adapter's driver as the log names it: the driver and its version
/// where the API reports them, and "driver unknown" where it does not -
/// Direct3D 12 reports neither, and an empty "driver )" read as a fault.
/// https://github.com/jub0t/Concat/issues/223
pub fn driver_of(info: &wgpu::AdapterInfo) -> String {
    let named = [info.driver.trim(), info.driver_info.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if named.is_empty() {
        "driver unknown".to_owned()
    } else {
        format!("driver {named}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(name: &str, device_type: wgpu::DeviceType, vendor: u32) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: name.into(),
            vendor,
            ..wgpu::AdapterInfo::new(device_type, wgpu::Backend::Metal)
        }
    }

    #[test]
    fn an_amd_gpu_beside_an_integrated_one_is_found_by_name_on_metal() {
        // What Metal lists on a 2015 MacBook Pro: no vendor ids.
        let mac = [
            adapter("AMD Radeon R9 M370X", wgpu::DeviceType::DiscreteGpu, 0),
            adapter(
                "Intel Iris Pro Graphics",
                wgpu::DeviceType::IntegratedGpu,
                0,
            ),
        ];
        assert!(amd_beside_integrated(&mac));
    }

    #[test]
    fn an_amd_gpu_is_found_by_its_vendor_id() {
        let pc = [
            adapter("Discrete", wgpu::DeviceType::DiscreteGpu, 0x1002),
            adapter("Integrated", wgpu::DeviceType::IntegratedGpu, 0),
        ];
        assert!(amd_beside_integrated(&pc));
    }

    #[test]
    fn other_gpus_keep_the_discrete_one() {
        let nvidia = [
            adapter("NVIDIA GeForce GT 750M", wgpu::DeviceType::DiscreteGpu, 0),
            adapter(
                "Intel Iris Pro Graphics",
                wgpu::DeviceType::IntegratedGpu,
                0,
            ),
        ];
        assert!(!amd_beside_integrated(&nvidia));
        let amd_alone = [adapter(
            "AMD Radeon Pro 560X",
            wgpu::DeviceType::DiscreteGpu,
            0,
        )];
        assert!(!amd_beside_integrated(&amd_alone));
    }
}
