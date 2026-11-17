mod grid;

use std::sync::Arc;

use web_time::Instant;

use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler, event::{KeyEvent, MouseButton, WindowEvent}, event_loop::{ActiveEventLoop, EventLoop}, keyboard::{KeyCode, PhysicalKey}, window::Window,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::EventLoopExtWebSys;

enum SimulationState {
    Running,
    AdvanceOnce,
    Stopped
}

struct TimeInfo {
    pub delta_time: f32,
    pub avg_fps: f32,
    time: Instant,
    generation_time: Instant,
    dt_sum: f32,
    frame_count: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
}

impl TimeInfo {
    pub fn new() -> Self {
        Self {
            delta_time: 0.0,
            avg_fps: 0.0,
            time: Instant::now(),
            generation_time: Instant::now(),
            dt_sum: 0.0,
            frame_count: 0.0,
        }
    }

    pub fn update(&mut self) {
        self.delta_time = self.time.elapsed().as_secs_f32();
        self.time = Instant::now();

        self.dt_sum += self.delta_time;
        self.frame_count += 1.0;
        if self.dt_sum > 1.0 {
            self.avg_fps = self.frame_count / self.dt_sum;
            self.frame_count = 0.0;
            self.dt_sum = 0.0;
        }
    }

    pub fn new_generation(&mut self, generation_per_second: u32) -> bool {
        if self.generation_time.elapsed().as_secs_f32() > 1.0 / generation_per_second as f32 {
            self.generation_time = Instant::now();
            true
        } else {
            false
        }
    }

    pub fn get_delta_time(&self) -> f32 {
        self.delta_time
    }

    pub fn get_fps(&self) -> f32 {
        self.avg_fps
    }
}

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    window: Arc<Window>,
    render_pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    grid: grid::Grid,
    //update_simulation: bool,
    simulation_state: SimulationState,
    generation_per_second: u32,
    time_info: TimeInfo,
    cursor_position: winit::dpi::PhysicalPosition<f64>,
    camera: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    is_panning: bool,
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    egui_wgpu: egui_wgpu::Renderer,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::PRIMARY,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    //wgpu::Limits::downlevel_webgl2_defaults()
                    wgpu::Limits::defaults()
                } else {
                    wgpu::Limits::defaults()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Render Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let grid = grid::Grid::new(
            (512, 512),
            &device,
            &render_bind_group_layout,
            &compute_bind_group_layout,
        );

        grid.random_grid(&queue);


        let simulation_state = SimulationState::Stopped;

        let generation_per_second = 2;

        let time_info = TimeInfo::new();

        let cursor_position = winit::dpi::PhysicalPosition::new(0.0, 0.0);

        let camera = CameraUniform {
            offset: [0.0, 0.0],
            scale: [2.0, 2.0],
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }]
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }]
        });

        let render_shader = device.create_shader_module(wgpu::include_wgsl!("render.wgsl"));
        let render_pipeline_layout = {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[Some(&render_bind_group_layout), Some(&camera_bind_group_layout)],
                immediate_size: 0,
            })
        };
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let compute_shader = device.create_shader_module(wgpu::include_wgsl!("compute.wgsl"));
        let compute_pipeline_layout = {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[Some(&compute_bind_group_layout)],
                immediate_size: 0,
            })
        };
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: None,
            compilation_options: Default::default(),
            cache: Default::default(),
        });

        let egui_ctx = egui::Context::default();
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_wgpu = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            window,
            render_pipeline,
            compute_pipeline,
            grid,
            simulation_state,
            generation_per_second,
            time_info,
            camera,
            camera_buffer,
            camera_bind_group,
            is_panning: false,
            cursor_position,
            egui_ctx,
            egui_winit,
            egui_wgpu,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            // webGL max is 2048
            //let max = 2048;
            self.config.width = width/*.min(max)*/;
            self.config.height = height/*.min(max)*/;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;
        }
    }

    pub fn handle_key(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, is_pressed: bool) {
        match (code, is_pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            (KeyCode::Space, true) => self.simulation_state = SimulationState::AdvanceOnce,
            (KeyCode::ArrowRight, true) => self.simulation_state = SimulationState::Running,
            (KeyCode::ArrowLeft, true) => self.simulation_state = SimulationState::Stopped,
            _ => (),
        }
    }

    pub fn handle_touch(&mut self, is_screen_touched: bool) {
        if is_screen_touched {
            self.simulation_state = SimulationState::AdvanceOnce;
        }
    }

    pub fn handle_mouse(&mut self, button: MouseButton, mouse_state: winit::event::ElementState) {
        match button {
            MouseButton::Left => {
                let grid_size = self.grid.get_grid_size();

                // let x = (self.cursor_position.x / self.config.width as f64 * grid_size.width as f64) as u32;
                // let y = (self.cursor_position.y / self.config.height as f64 * grid_size.height as f64) as u32;
                let width = self.config.width as f64;
                let height = self.config.height as f64;

                let ndc_x = (self.cursor_position.x / width) * 2.0 - 1.0;
                let ndc_y = 1.0 - (self.cursor_position.y / height) * 2.0;

                let pos_x = (ndc_x - self.camera.offset[0] as f64) / self.camera.scale[0] as f64;
                let pos_y = (ndc_y - self.camera.offset[1] as f64) / self.camera.scale[1] as f64;

                let uv_x = (pos_x + 1.0) / 2.0;
                let uv_y = (1.0 - pos_y) / 2.0;

                let x = (uv_x * grid_size.width as f64).floor();
                let y = (uv_y * grid_size.height as f64).floor();

                if x >= 0.0 && y >= 0.0 && x < grid_size.width as f64 && y < grid_size.height as f64 {
                    self.grid.set_cell_alive(x as u32, y as u32, &self.queue);
                    self.window.request_redraw();
                }
            }
            MouseButton::Right => {
                self.is_panning = mouse_state == winit::event::ElementState::Pressed;
            }
            _ => {}
        }
    }

    pub fn handle_mouse_mouvement(&mut self, position: winit::dpi::PhysicalPosition<f64>) {
        let old_pos = self.cursor_position;
        self.cursor_position = position;

        if self.is_panning {
            let dx = (position.x - old_pos.x) as f32;
            let dy = (position.y - old_pos.y) as f32;

            self.camera.offset[0] += (dx / self.config.width as f32) * 2.0;
            self.camera.offset[1] -= (dy / self.config.height as f32) * 2.0;

            self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera]));
            self.window.request_redraw();
        }
    }

    pub fn handle_mouse_wheel(&mut self, delta: winit::event::MouseScrollDelta) {
        let zoom = match delta {
            winit::event::MouseScrollDelta::LineDelta(_, y) => {
                let zoom = y * 0.1;
                self.camera.scale[0] += zoom;
                self.camera.scale[1] += zoom;
            }
            winit::event::MouseScrollDelta::PixelDelta(pos) => {
                let dx = pos.x as f32;
                let dy = pos.y as f32;

                self.camera.offset[0] += (dx / self.config.width as f32) * 2.0;
                self.camera.offset[1] -= (dy / self.config.height as f32) * 2.0;
            },
        };

        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera]));
        self.window.request_redraw();
    }

    pub fn calculate_simulation(&mut self) {
        let mut encoder = self.device.create_command_encoder(&Default::default());

        let bind_group = self.grid.get_compute_bind_group();

        let (workgroup_x, workgroup_y) = {
            let size = self.grid.get_grid_size();
            ((size.width + 7) / 8, (size.height + 7) / 8)
        };

        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroup_x, workgroup_y, 1);
        }

        self.grid.increase_generation();

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    pub fn update(&mut self) {
        match self.simulation_state {
            SimulationState::AdvanceOnce => {
                self.calculate_simulation();
                self.simulation_state = SimulationState::Stopped;
            }
            SimulationState::Running => {
                if self.time_info.new_generation(self.generation_per_second) {
                    self.calculate_simulation();
                }
            }
            SimulationState::Stopped => {},
        }
    }

    pub fn build_ui(&mut self, command_encoder: &mut wgpu::CommandEncoder) -> (Vec<egui::ClippedPrimitive>, egui_wgpu::ScreenDescriptor, egui::TexturesDelta) {
        let raw_inputs = self.egui_winit.take_egui_input(&self.window);
        self.egui_ctx.begin_pass(raw_inputs);

        egui::Window::new("Grid").show(&self.egui_ctx, |ui| {
            if ui.button("Clear grid").clicked() {
                self.grid.clear(&self.queue);
            }
            if ui.button("Randomize grid").clicked() {
                self.grid.random_grid(&self.queue);
            }
        });

        egui::Window::new("Fps")
            .title_bar(false)
            .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
            .show(&self.egui_ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("FPS: {}", self.time_info.avg_fps as i32))
                )
            });

        let (play_button, next_state) = match self.simulation_state {
            SimulationState::Running { .. } => ("|| Pause", SimulationState::Stopped),
            SimulationState::Stopped | SimulationState::AdvanceOnce => ("> Play", SimulationState::Running),
        };
        let mut generation_per_second = self.generation_per_second;
        egui::Window::new("Controls")
            .title_bar(false)
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -10.0])
            .show(&self.egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(play_button).clicked() {
                        self.simulation_state = next_state;
                    }
                    ui.add(
                        egui::Slider::new(&mut generation_per_second, 1..=100)
                        .orientation(egui::SliderOrientation::Horizontal)
                        //.text("generation per second")
                    );
                })
            });
        self.generation_per_second = generation_per_second;

        let full_output = self.egui_ctx.end_pass();
        self.egui_winit.handle_platform_output(&self.window, full_output.platform_output);
        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, self.window.scale_factor() as f32);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            for delta in image_delta {
                self.egui_wgpu.update_texture(&self.device, &self.queue, *id, delta);
            }
        }
        self.egui_wgpu.update_buffers(&self.device, &self.queue, command_encoder, &paint_jobs, &screen_descriptor);

        (paint_jobs, screen_descriptor, full_output.textures_delta)
    }

    pub fn render(&mut self) -> anyhow::Result<()> {
        self.window.request_redraw();

        if !self.is_surface_configured {
            return Ok(());
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => anyhow::bail!("Lost device"),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let (paint_jobs, screen_descriptor, mut textures_delta) = self.build_ui(&mut encoder);

        // game pass
        let grid_bind_group = self.grid.get_render_bind_group();
        {
            let game_color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })];

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Game Pass"),
                color_attachments: &game_color_attachments,
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, grid_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        // egui pass
        {   
            let ui_color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })];

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Pass"),
                color_attachments: &ui_color_attachments,
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            }).forget_lifetime();
            self.egui_wgpu.render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);

        for id in &textures_delta.free {
            self.egui_wgpu.free_texture(id);
        }
        textures_delta.clear();

        self.time_info.update();

        Ok(())
    }
}

pub struct App {
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<State>>,
    state: Option<State>,
}

impl App {
    pub fn new(#[cfg(target_arch = "wasm32")] event_loop: &EventLoop<State>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let proxy = Some(event_loop.create_proxy());
        Self {
            state: None,
            #[cfg(target_arch = "wasm32")]
            proxy,
        }
    }
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes();

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            const CANVAS_ID: &str = "canvas";

            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas_element = canvas.unchecked_into();
            window_attributes = window_attributes.with_canvas(Some(html_canvas_element));
        }

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.state = Some(pollster::block_on(State::new(window)).unwrap());
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(proxy) = self.proxy.take() {
                wasm_bindgen_futures::spawn_local(async move {
                    assert!(
                        proxy
                            .send_event(
                                State::new(window)
                                    .await
                                    .expect("Unable to create canvas!!!")
                            )
                            .is_ok()
                    )
                });
            }
        }
    }

    fn user_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        #[cfg(target_arch = "wasm32")] mut event: State,
        #[cfg(not(target_arch = "wasm32"))] event: State,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            event.window.request_redraw();
            event.resize(
                event.window.inner_size().width,
                event.window.inner_size().height,
            );
        }
        self.state = Some(event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };

        let ui_event_response = state.egui_winit.on_window_event(&state.window, &event);
        if ui_event_response.consumed {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => state.resize(size.width, size.height),
                WindowEvent::RedrawRequested => {
                    state.update();
                    match state.render() {
                        Ok(_) => (),
                        Err(e) => {
                            log::error!("{e}");
                            event_loop.exit();
                        }
                    }
                }
                _ => ()
            }
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                state.update();
                match state.render() {
                    Ok(_) => (),
                    Err(e) => {
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed()),
            WindowEvent::MouseInput { state: mouse_state, button, .. } => state.handle_mouse(button, mouse_state),
            WindowEvent::Touch(touch) => state.handle_touch(
                touch.phase == winit::event::TouchPhase::Started
                    || touch.phase == winit::event::TouchPhase::Moved,
            ),
            WindowEvent::CursorMoved { position, .. } => state.handle_mouse_mouvement(position),
            WindowEvent::MouseWheel { delta, .. } => state.handle_mouse_wheel(delta),
            _ => {}
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }

    #[cfg(target_arch = "wasm32")]
    {
        console_log::init_with_level(log::Level::Info).unwrap_throw();
    }

    let event_loop = EventLoop::with_user_event().build()?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App::new();
        event_loop.run_app(&mut app)?;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let app = App::new(&event_loop);
        event_loop.spawn_app(app);
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    run().unwrap_throw();

    Ok(())
}
