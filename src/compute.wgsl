@binding(0) @group(0) var grid_in: texture_2d<f32>;
@binding(1) @group(0) var grid_out: texture_storage_2d<rgba8unorm, write>;

// get if a cell at coordinates x y is alive and wraps around the edges
fn cell(x: i32, y: i32) -> i32 {
    let dimensions = textureDimensions(grid_in);

    // wrap
    let wrapped_x = (x + i32(dimensions.x)) % i32(dimensions.x);
    let wrapped_y = (y + i32(dimensions.y)) % i32(dimensions.y);

    let color = textureLoad(grid_in, vec2<i32>(wrapped_x, wrapped_y), 0);

    // if the red channel is bright the cell is alive, otherwise it's dead
    if color.r > 0.5 {
        return 1;
    }
    return 0;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dimensions = textureDimensions(grid_in);
    let x = i32(gid.x);
    let y = i32(gid.y);

    if x >= i32(dimensions.x) || y >= i32(dimensions.y) {
        return;
    }

    // count the the 8 surrounding alive neighbors
    let neighbors = cell(x - 1, y - 1) + cell(x, y - 1) + cell(x + 1, y - 1) +
        cell(x - 1, y) + cell(x + 1, y) +
        cell(x - 1, y + 1) + cell(x, y + 1) + cell(x + 1, y + 1);

    let is_alive = cell(x, y) == 1;
    var next_state = 0;
    if is_alive && neighbors == 2 {
        next_state = 1;
    } else if neighbors == 3 {
        next_state = 1;
    }

    let color = f32(next_state);
    textureStore(grid_out, vec2<i32>(x, y), vec4<f32>(color, color, color, 1.0));
}
