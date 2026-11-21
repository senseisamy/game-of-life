pub const ALIVE_COLOR: [u8; 4] = [255; 4];
pub const DEAD_COLOR: [u8; 4] = [15, 15, 15, 255];

pub struct Grid {
    texture_a: wgpu::Texture,
    render_bind_group_a: wgpu::BindGroup,
    compute_bind_group_a: wgpu::BindGroup,
    texture_b: wgpu::Texture,
    render_bind_group_b: wgpu::BindGroup,
    compute_bind_group_b: wgpu::BindGroup,
    generation: u64,
    alive_cell_color: [u8; 4],
}

impl Grid {
    pub fn new(
        dimensions: (u32, u32),
        device: &wgpu::Device,
        render_bind_group_layout: &wgpu::BindGroupLayout,
        compute_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture_desc = wgpu::TextureDescriptor {
            label: Some("Grid Texture Descriptor"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        // We need two textures for the compute shader, the texture A is the current grid state and the texture B is the future grid state after updating the grid
        let texture_a = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture A"),
            ..texture_desc
        });
        let texture_b = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture B"),
            ..texture_desc
        });
        let view_a = texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let view_b = texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Grid Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let render_bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let render_bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let compute_bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view_b),
                },
            ],
        });
        let compute_bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view_a),
                },
            ],
        });

        Self {
            texture_a,
            render_bind_group_a,
            compute_bind_group_a,
            texture_b,
            render_bind_group_b,
            compute_bind_group_b,
            generation: 0,
            alive_cell_color: ALIVE_COLOR,
        }
    }

    // fills the grid with dead cells (black)
    #[allow(unused)]
    pub fn clear(&self, queue: &wgpu::Queue) {
        let texture = if self.is_generation_even() {
            &self.texture_a
        } else {
            &self.texture_b
        };

        let texture_size = texture.size();

        let total_pixels = (texture_size.width * texture_size.height) as usize;

        let data: Vec<u8> = DEAD_COLOR.repeat(total_pixels);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(texture_size.width * 4),
                rows_per_image: Some(texture_size.height),
            },
            texture_size,
        )
    }

    // fills the grid with random cell generation
    pub fn random_grid(&self, queue: &wgpu::Queue) {
        let texture = if self.is_generation_even() {
            &self.texture_a
        } else {
            &self.texture_b
        };

        let texture_size = texture.size();

        let total_pixels = (texture_size.width * texture_size.height) as usize;

        let mut data: Vec<u8> = Vec::with_capacity(total_pixels * 4);

        for _ in 0..total_pixels {
            match rand::random_bool(0.2) {
                // 20% of being true
                true => data.extend_from_slice(&self.alive_cell_color),
                false => data.extend_from_slice(&DEAD_COLOR),
            }
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(texture_size.width * 4),
                rows_per_image: Some(texture_size.height),
            },
            texture_size,
        )
    }

    #[allow(unused)]
    pub fn set_cell_alive(&self, x: u32, y: u32, queue: &wgpu::Queue) {
        let texture = if self.is_generation_even() {
            &self.texture_a
        } else {
            &self.texture_b
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &self.alive_cell_color,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn get_grid_size(&self) -> wgpu::Extent3d {
        self.texture_a.size()
    }

    pub fn is_generation_even(&self) -> bool {
        self.generation % 2 == 0
    }

    pub fn get_render_bind_group(&self) -> &wgpu::BindGroup {
        if self.is_generation_even() {
            &self.render_bind_group_a
        } else {
            &self.render_bind_group_b
        }
    }

    pub fn get_compute_bind_group(&self) -> &wgpu::BindGroup {
        if self.is_generation_even() {
            &self.compute_bind_group_a
        } else {
            &self.compute_bind_group_b
        }
    }

    pub fn increase_generation(&mut self) {
        self.generation += 1;
    }
}
