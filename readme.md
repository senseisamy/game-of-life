# Conway's Game of Life

This is a little personal project of recreating Conway's Game of Life in rust. I used this project to teach myself how to use compute shaders (thanks to the [learn wgpu](https://sotrh.github.io/learn-wgpu/) guide): the simulation runs on the gpu meaning every pixel calculation runs in parallel. This also works on the web with webassembly, you can check it out here: https://game-of-life.jsp.lol/  
I used [wgpu](https://github.com/gfx-rs/wgpu) for the graphics API, [egui](https://github.com/emilk/egui) for the UI and [winit](https://github.com/rust-windowing/winit) for the window and inputs management.

## Compilation

### Linux, MacOS and Windows

Simply run ```cargo run -r```

### Web (wasm32)

I use [wasm-pack](https://wasm-bindgen.github.io/wasm-pack/) for simplicity, once installed run ```wasm-pack build --target web```  
You'll then have to import the WASM module in javascript:
```js
const init = await import('./pkg/game_of_life.js');
init().then(() => console.log("WASM Loaded"))
```
