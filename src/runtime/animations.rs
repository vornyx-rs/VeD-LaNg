/// Deterministic spring animation engine with feature-gated backend selection.
///
/// Current behavior:
/// - CPU backend is fully implemented and deterministic.
/// - WebGPU backend is a feature-gated stub (for incremental rollout).
///
/// This design allows us to ship and test animation semantics now while
/// introducing GPU acceleration safely in later iterations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringConfig {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
    pub precision: f64,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            stiffness: 120.0,
            damping: 14.0,
            mass: 1.0,
            precision: 0.001,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationState {
    pub position: f64,
    pub velocity: f64,
}

impl AnimationState {
    pub fn new(from: f64) -> Self {
        Self {
            position: from,
            velocity: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationFrame {
    pub value: f64,
    pub done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AnimationBackend {
    Cpu,
    #[cfg(feature = "webgpu")]
    WebGpu,
}

#[cfg(feature = "webgpu")]
mod webgpu {
    use std::sync::OnceLock;

    use wgpu::util::DeviceExt;

    pub(super) struct WebGpuContext {
        pub device: wgpu::Device,
        pub queue: wgpu::Queue,
        pub pipeline: wgpu::ComputePipeline,
        pub bind_group_layout: wgpu::BindGroupLayout,
    }

    const WGSL: &str = r#"
        struct Params {
            target_pos: f32,
            stiffness: f32,
            damping: f32,
            mass: f32,
            dt: f32,
            _pad0: f32,
            _pad1: f32,
            _pad2: f32,
        };

        @group(0) @binding(0) var<storage, read_write> positions: array<f32>;
        @group(0) @binding(1) var<storage, read_write> velocities: array<f32>;
        @group(0) @binding(2) var<storage, read> params: Params;

        @compute @workgroup_size(1)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
            let i = global_id.x;
            let pos = positions[i];
            let vel = velocities[i];
            let force = -params.stiffness * (pos - params.target_pos) - params.damping * vel;
            let acceleration = force / params.mass;
            let next_vel = vel + acceleration * params.dt;
            let next_pos = pos + next_vel * params.dt;
            velocities[i] = next_vel;
            positions[i] = next_pos;
        }
    "#;

    fn build_context() -> Option<WebGpuContext> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("ved-animation-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        ))
        .ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ved-spring-shader"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ved-spring-bind-group"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ved-spring-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ved-spring-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        Some(WebGpuContext {
            device,
            queue,
            pipeline,
            bind_group_layout,
        })
    }

    pub(super) fn context() -> Option<&'static WebGpuContext> {
        static CONTEXT: OnceLock<Option<WebGpuContext>> = OnceLock::new();
        CONTEXT.get_or_init(|| build_context()).as_ref()
    }

    pub(super) fn to_bytes(values: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * 4);
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    pub(super) fn read_f32(buffer: &wgpu::Buffer, device: &wgpu::Device) -> f32 {
        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        let _ = receiver.recv();
        let data = slice.get_mapped_range();
        let mut raw = [0u8; 4];
        raw.copy_from_slice(&data[..4]);
        drop(data);
        buffer.unmap();
        f32::from_le_bytes(raw)
    }

    pub(super) fn create_storage_buffer(
        device: &wgpu::Device,
        bytes: &[u8],
        label: &str,
    ) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        })
    }

    pub(super) fn create_params_buffer(device: &wgpu::Device, bytes: &[u8]) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ved-spring-params"),
            contents: bytes,
            usage: wgpu::BufferUsages::STORAGE,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub struct AnimationEngine {
    backend: AnimationBackend,
}

#[allow(dead_code)]
impl AnimationEngine {
    pub fn new(backend: AnimationBackend) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> AnimationBackend {
        self.backend
    }

    pub fn best_available() -> Self {
        #[cfg(feature = "webgpu")]
        {
            if webgpu::context().is_some() {
                Self::new(AnimationBackend::WebGpu)
            } else {
                Self::new(AnimationBackend::Cpu)
            }
        }

        #[cfg(not(feature = "webgpu"))]
        {
            Self::new(AnimationBackend::Cpu)
        }
    }

    pub fn supports_acceleration(&self) -> bool {
        #[cfg(feature = "webgpu")]
        {
            self.backend == AnimationBackend::WebGpu && webgpu::context().is_some()
        }

        #[cfg(not(feature = "webgpu"))]
        {
            false
        }
    }

    /// Execute one spring step using the selected backend.
    ///
    /// WebGPU currently falls back to CPU simulation until compute pipeline
    /// integration is added in Phase 5.2.
    pub fn step(
        &self,
        state: &mut AnimationState,
        target: f64,
        config: SpringConfig,
        dt: f64,
    ) -> AnimationFrame {
        match self.backend {
            AnimationBackend::Cpu => Self::step_spring_cpu(state, target, config, dt),
            #[cfg(feature = "webgpu")]
            AnimationBackend::WebGpu => Self::step_spring_webgpu(state, target, config, dt),
        }
    }

    /// Advance a spring simulation by one time step.
    pub fn step_spring(
        state: &mut AnimationState,
        target: f64,
        config: SpringConfig,
        dt: f64,
    ) -> AnimationFrame {
        Self::step_spring_cpu(state, target, config, dt)
    }

    fn step_spring_cpu(
        state: &mut AnimationState,
        target: f64,
        config: SpringConfig,
        dt: f64,
    ) -> AnimationFrame {
        let safe_dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            1.0 / 60.0
        };
        let safe_mass = if config.mass <= 0.0 {
            0.0001
        } else {
            config.mass
        };

        let force = -config.stiffness * (state.position - target) - config.damping * state.velocity;
        let acceleration = force / safe_mass;

        state.velocity += acceleration * safe_dt;
        state.position += state.velocity * safe_dt;

        let done = Self::is_done(state, target, config.precision);

        if done {
            state.position = target;
            state.velocity = 0.0;
        }

        AnimationFrame {
            value: state.position,
            done,
        }
    }

    fn is_done(state: &AnimationState, target: f64, precision: f64) -> bool {
        state.velocity.abs() < precision && (state.position - target).abs() < precision
    }

    #[cfg(feature = "webgpu")]
    fn step_spring_webgpu(
        state: &mut AnimationState,
        target: f64,
        config: SpringConfig,
        dt: f64,
    ) -> AnimationFrame {
        let Some(context) = webgpu::context() else {
            return Self::step_spring_cpu(state, target, config, dt);
        };

        let safe_dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            1.0 / 60.0
        };
        let safe_mass = if config.mass <= 0.0 {
            0.0001
        } else {
            config.mass
        };

        let device = &context.device;
        let queue = &context.queue;

        let position_bytes = webgpu::to_bytes(&[state.position as f32]);
        let velocity_bytes = webgpu::to_bytes(&[state.velocity as f32]);
        let params_bytes = webgpu::to_bytes(&[
            target as f32,
            config.stiffness as f32,
            config.damping as f32,
            safe_mass as f32,
            safe_dt as f32,
            0.0,
            0.0,
            0.0,
        ]);

        let positions =
            webgpu::create_storage_buffer(device, &position_bytes, "ved-spring-position");
        let velocities =
            webgpu::create_storage_buffer(device, &velocity_bytes, "ved-spring-velocity");
        let params = webgpu::create_params_buffer(device, &params_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ved-spring-bind-group"),
            layout: &context.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: positions.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: velocities.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ved-spring-encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ved-spring-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&context.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }

        let position_readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ved-spring-position-read"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let velocity_readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ved-spring-velocity-read"),
            size: 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(&positions, 0, &position_readback, 0, 4);
        encoder.copy_buffer_to_buffer(&velocities, 0, &velocity_readback, 0, 4);

        queue.submit(Some(encoder.finish()));
        device.poll(wgpu::Maintain::Wait);

        let next_pos = webgpu::read_f32(&position_readback, device);
        let next_vel = webgpu::read_f32(&velocity_readback, device);

        state.position = next_pos as f64;
        state.velocity = next_vel as f64;

        let done = Self::is_done(state, target, config.precision);
        if done {
            state.position = target;
            state.velocity = 0.0;
        }

        AnimationFrame {
            value: state.position,
            done,
        }
    }

    /// Simulate a spring until it settles, returning sampled values.
    pub fn settle_spring(
        from: f64,
        target: f64,
        config: SpringConfig,
        max_steps: usize,
    ) -> Vec<f64> {
        let engine = Self::best_available();
        engine.settle(from, target, config, max_steps)
    }

    pub fn settle(
        &self,
        from: f64,
        target: f64,
        config: SpringConfig,
        max_steps: usize,
    ) -> Vec<f64> {
        let mut state = AnimationState::new(from);
        let mut values = Vec::with_capacity(max_steps.saturating_add(1));
        values.push(from);

        for _ in 0..max_steps {
            let frame = self.step(&mut state, target, config, 1.0 / 60.0);
            values.push(frame.value);
            if frame.done {
                break;
            }
        }

        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_engine_uses_supported_backend() {
        let engine = AnimationEngine::best_available();
        #[cfg(feature = "webgpu")]
        {
            if engine.supports_acceleration() {
                assert_eq!(engine.backend(), AnimationBackend::WebGpu);
            } else {
                assert_eq!(engine.backend(), AnimationBackend::Cpu);
            }
        }
        #[cfg(not(feature = "webgpu"))]
        assert_eq!(engine.backend(), AnimationBackend::Cpu);
    }

    #[test]
    fn step_spring_moves_towards_target() {
        let mut state = AnimationState::new(0.0);
        let frame =
            AnimationEngine::step_spring(&mut state, 1.0, SpringConfig::default(), 1.0 / 60.0);
        assert!(frame.value > 0.0);
        assert!(!frame.done);
    }

    #[test]
    fn settle_spring_converges_near_target() {
        let cfg = SpringConfig::default();
        let values = AnimationEngine::settle_spring(0.0, 1.0, cfg, 600);
        let last = *values.last().expect("expected at least one value");
        assert!((last - 1.0).abs() <= cfg.precision * 2.0);
    }

    #[test]
    fn invalid_dt_falls_back_to_default_step() {
        let mut state = AnimationState::new(0.0);
        let engine = AnimationEngine::new(AnimationBackend::Cpu);
        let frame = engine.step(&mut state, 1.0, SpringConfig::default(), f64::NAN);
        assert!(frame.value > 0.0);
    }

    #[cfg(feature = "webgpu")]
    #[test]
    fn webgpu_step_matches_cpu_within_tolerance() {
        let engine = AnimationEngine::best_available();
        if !engine.supports_acceleration() {
            return;
        }

        let cfg = SpringConfig::default();
        let mut cpu_state = AnimationState::new(0.0);
        let mut gpu_state = AnimationState::new(0.0);

        let cpu_frame = AnimationEngine::step_spring(&mut cpu_state, 1.0, cfg, 1.0 / 60.0);
        let gpu_frame = engine.step(&mut gpu_state, 1.0, cfg, 1.0 / 60.0);

        let diff = (cpu_frame.value - gpu_frame.value).abs();
        assert!(diff <= 1.0e-3, "expected GPU parity, diff was {diff}");
    }
}
