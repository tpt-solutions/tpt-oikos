# tpt-chora — Human Observation Runtime

**Pillar:** Chora (Presentation Layer)
**Role:** Secure, GPU-accelerated rendering runtime for TPT Oikos

Chora replaces vulnerable HTML/JS browsers with a native GPU render graph
built on `wgpu`. It renders agent dashboards, contract state, mandates,
streaming payments, and governance interfaces directly via WebGPU pipelines.

## Architecture

```
chora-cli        Entry point — launches the renderer window
chora-render     Core wgpu render graph, scene graph, GPU pipeline setup
chora-ui         UI primitives (panels, text, charts, status indicators)
chora-data       Data binding layer — connects to koinon state
```

## Crates

| Crate | Description |
|-------|-------------|
| `chora-render` | wgpu abstraction: device setup, render graph, scene graph, shader management |
| `chora-ui` | Composable UI primitives for building agent dashboards |
| `chora-data` | Observable data bindings for mandates, balances, streaming payments |
| `chora-cli` | CLI binary that initializes the window and drives the render loop |

## Prerequisites

- Rust 1.74+ (MSRV)
- GPU with WebGPU/Vulkan/Metal support
- `wgpu` will select the appropriate backend at runtime

## Quick Start

```sh
cd pillars/chora
cargo run -p chora-cli
```

This launches a window rendering a colored triangle — the minimal starting
point. From here the render graph will grow to support panels, text, charts,
and full agent dashboard layouts.

## Rendering Pipeline

1. **Scene Graph** — tree of render nodes (geometry, transforms, visibility)
2. **Render Graph** — directed acyclic graph of render passes
3. **UI Layer** — layout engine composes panels/text/charts into the scene
4. **Data Bindings** — live state from koinon feeds into UI nodes each frame

## License

Apache-2.0 OR MIT
