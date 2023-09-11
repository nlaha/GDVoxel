use bytemuck::*;
use futures_intrusive::channel::shared::oneshot_channel;
use godot::prelude::Vector3;
use ndshape::RuntimeShape;
use ndshape::Shape;
use std::{borrow::Cow, str::FromStr};
use wgpu::util::DeviceExt;

pub async fn execute_gpu(
    shape_3d: RuntimeShape<u32, 3>,
    offsets: Vector3,
    generated_size: RuntimeShape<u32, 3>,
) -> Option<Vec<f32>> {
    // Instantiates instance of WebGPU
    let instance = wgpu::Instance::default();

    // `request_adapter` instantiates the general connection to the GPU
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await?;

    // `request_device` instantiates the feature specific connection to the GPU, defining some parameters,
    //  `features` being the available features.
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        )
        .await
        .unwrap();

    let info = adapter.get_info();
    // skip this on LavaPipe temporarily
    if info.vendor == 0x10005 {
        return None;
    }

    execute_gpu_inner(&device, &queue, shape_3d, offsets, generated_size).await
}

async fn execute_gpu_inner(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shape_3d: RuntimeShape<u32, 3>,
    offsets: Vector3,
    generated_size: RuntimeShape<u32, 3>,
) -> Option<Vec<f32>> {
    // Loads the shader from WGSL
    let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("noise.wgsl"))),
    });

    // Instantiates buffer without data.
    // `usage` of buffer specifies how it can be used:
    //   `BufferUsages::MAP_READ` allows it to be read (outside the shader).
    //   `BufferUsages::COPY_DST` allows it to be the destination of the copy.
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (shape_3d.size() as usize * std::mem::size_of::<f32>()) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // create texture
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(&format!(
            "[GDVoxel Compute] Terrain noise [{}, {}, {}]",
            offsets.x, offsets.y, offsets.z
        )),
        size: wgpu::Extent3d {
            width: shape_3d.as_array()[0],
            height: shape_3d.as_array()[1],
            depth_or_array_layers: shape_3d.as_array()[2],
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some(&format!(
            "[GDVoxel Compute] Terrain noise [{}, {}, {}]",
            offsets.x, offsets.y, offsets.z
        )),
        format: Some(wgpu::TextureFormat::R32Float),
        base_mip_level: 0,
        mip_level_count: Some(1),
        ..Default::default()
    });

    // A bind group defines how buffers are accessed by shaders.
    // It is to WebGPU what a descriptor set is to Vulkan.
    // `binding` here refers to the `binding` of a buffer in the shader (`layout(set = 0, binding = 0) buffer`).

    // A pipeline specifies the operation of a shader

    // Instantiates the pipeline.
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &cs_module,
        entry_point: "main",
    });

    // Instantiates the bind group, once again specifying the binding of buffers.
    let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            // frequency uniform
            wgpu::BindGroupEntry {
                binding: 1,
                resource: device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Terrain noise settings"),
                        contents: bytemuck::cast_slice(&[
                            0.01f32,
                            ((offsets.x as f32 / shape_3d.as_array()[0] as f32)
                                * (shape_3d.as_array()[0] as f32 * 0.01) as f32),
                            ((offsets.y as f32 / shape_3d.as_array()[1] as f32)
                                * (shape_3d.as_array()[1] as f32 * 0.01) as f32),
                            ((offsets.z as f32 / shape_3d.as_array()[2] as f32)
                                * (shape_3d.as_array()[2] as f32 * 0.01) as f32),
                            generated_size.as_array()[0] as f32,
                            generated_size.as_array()[1] as f32,
                            generated_size.as_array()[2] as f32,
                        ]),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    })
                    .as_entire_binding(),
            },
        ],
    });

    // A command encoder executes one or many pipelines.
    // It is to WebGPU what a command buffer is to Vulkan.
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.insert_debug_marker("[GDVoxel Compute] Terrain noise");
        cpass.dispatch_workgroups(
            shape_3d.as_array()[0],
            shape_3d.as_array()[1],
            shape_3d.as_array()[2],
        );
        // Number of cells to run, the (x,y,z) size of item being processed
    }
    // Sets adds copy operation to command encoder.
    // Will copy data from storage buffer on GPU to staging buffer on CPU.
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(std::mem::size_of::<f32>() as u32 * shape_3d.as_array()[0]),
                rows_per_image: Some(shape_3d.as_array()[1]),
            },
        },
        wgpu::Extent3d {
            width: shape_3d.as_array()[0],
            height: shape_3d.as_array()[1],
            depth_or_array_layers: shape_3d.as_array()[2],
        },
    );

    // Submits command encoder for processing
    queue.submit(Some(encoder.finish()));

    // Note that we're not calling `.await` here.
    let buffer_slice = staging_buffer.slice(..);
    // Sets the buffer up for mapping, sending over the result of the mapping back to us when it is finished.
    let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    // Poll the device in a blocking manner so that our future resolves.
    // In an actual application, `device.poll(...)` should
    // be called in an event loop or on another thread.
    device.poll(wgpu::Maintain::Wait);

    // Awaits until `buffer_future` can be read from
    if let Some(Ok(())) = receiver.receive().await {
        // Gets contents of buffer
        let data = buffer_slice.get_mapped_range();
        // Since contents are got in bytes, this converts these bytes back to f32
        let result = bytemuck::cast_slice(&data).to_vec();

        // With the current interface, we have to make sure all mapped views are
        // dropped before we unmap the buffer.
        drop(data);
        staging_buffer.unmap(); // Unmaps buffer from memory
                                // If you are familiar with C++ these 2 lines can be thought of similarly to:
                                //   delete myPointer;
                                //   myPointer = NULL;
                                // It effectively frees the memory

        // Returns data from buffer
        Some(result)
    } else {
        panic!("failed to run compute on gpu!")
    }
}
