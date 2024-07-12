use burn_cube::prelude::*;

use super::{
    base::{Dimensions, Offsets, SharedMemories},
    config::CmmaConfig,
};

#[cube]
pub(crate) fn load_to_shared_memories<F: Float, FC: Float>(
    lhs: &Tensor<F>,
    rhs: &Tensor<F>,
    offsets: Offsets,
    mut shared: SharedMemories<F, FC>,
    config: Comptime<CmmaConfig>,
    dims: Dimensions,
) {
    let block_size_m = Comptime::map(config, |c| c.block_size_m);
    let block_size_k = Comptime::map(config, |c| c.block_size_k);
    let block_size_n = Comptime::map(config, |c| c.block_size_n);
    let tile_size = Comptime::map(config, |c| c.tile_size);
    let sm_vec = Comptime::map(config, |c| c.sm_vec);

    let num_tiles_in_k = Comptime::runtime(block_size_k / tile_size); // 2
    let tile_size_r = Comptime::runtime(tile_size); // 16
    let sm_vec_r = Comptime::runtime(sm_vec); // 4
    let lhs_sm_stride = Comptime::runtime(block_size_m / sm_vec); // 16
    let rhs_sm_stride = Comptime::runtime(block_size_n / sm_vec); // 16

    let lhs_stride = dims.k;
    let rhs_stride = dims.n;

    let within_tile_row_offset = SUBCUBE_DIM / sm_vec_r; // assuming subcube_dim is 32 -> 8
    let within_sm_row_offset = SUBCUBE_DIM / Comptime::runtime(tile_size); // assuming subcube_dim is 32 -> 2
    let subcube_id = UNIT_POS_X;
    let id_within_subcube = UNIT_POS_Y;

    // READ POSITIONS

    // There are two because 32 / 16. TODO generalize
    let unit_read_row_0 = id_within_subcube / sm_vec_r;
    let unit_read_row_1 = unit_read_row_0 + within_tile_row_offset;
    let unit_read_col = id_within_subcube % sm_vec_r;

    // LHS
    let lhs_tile_row = subcube_id / num_tiles_in_k;
    let lhs_tile_col = subcube_id % num_tiles_in_k;

    let lhs_offset = offsets.batch_lhs + offsets.k + offsets.cube_row * lhs_stride;
    let lhs_read_tile_offset =
        lhs_offset + lhs_tile_row * tile_size_r * lhs_stride + lhs_tile_col * tile_size_r;

    let lhs_read_pos_0 = lhs_read_tile_offset + unit_read_row_0 * lhs_stride + unit_read_col;
    let lhs_read_pos_1 = lhs_read_tile_offset + unit_read_row_1 * lhs_stride + unit_read_col;

    // RHS
    let rhs_tile_row = subcube_id % num_tiles_in_k;
    let rhs_tile_col = subcube_id / num_tiles_in_k;

    let rhs_offset = offsets.batch_rhs + offsets.k * rhs_stride + offsets.cube_col;
    let rhs_read_tile_offset =
        rhs_offset + rhs_tile_row * tile_size_r * rhs_stride + rhs_tile_col * tile_size_r;

    let rhs_read_pos_0 = rhs_read_tile_offset + unit_read_row_0 * rhs_stride + unit_read_col;
    let rhs_read_pos_1 = rhs_read_tile_offset + unit_read_row_1 * rhs_stride + unit_read_col;

    // WRITE POSITIONS

    // LHS
    let lhs_sm_row_offset = Comptime::runtime(tile_size * tile_size / block_size_m) * subcube_id; // 4

    let lhs_write_row_0 = id_within_subcube / lhs_sm_stride;
    let lhs_write_row_1 = lhs_write_row_0 + within_sm_row_offset;
    let lhs_write_col = id_within_subcube % lhs_sm_stride;

    let lhs_write_pos_0 = (lhs_sm_row_offset + lhs_write_row_0) * lhs_sm_stride + lhs_write_col;
    let lhs_write_pos_1 = (lhs_sm_row_offset + lhs_write_row_1) * lhs_sm_stride + lhs_write_col;

    // RHS
    let rhs_sm_row_offset = Comptime::runtime(tile_size * tile_size / block_size_n) * subcube_id; // 4

    let rhs_write_row_0 = id_within_subcube / rhs_sm_stride;
    let rhs_write_row_1 = rhs_write_row_0 + within_sm_row_offset;
    let rhs_write_col = id_within_subcube % rhs_sm_stride;

    let rhs_write_pos_0 = (rhs_sm_row_offset + rhs_write_row_0) * rhs_sm_stride + rhs_write_col;
    let rhs_write_pos_1 = (rhs_sm_row_offset + rhs_write_row_1) * rhs_sm_stride + rhs_write_col;

    // READ/WRITE

    shared.lhs[lhs_write_pos_0] = FC::cast_from(lhs[lhs_read_pos_0]);
    shared.lhs[lhs_write_pos_1] = FC::cast_from(lhs[lhs_read_pos_1]);

    shared.rhs[rhs_write_pos_0] = FC::cast_from(rhs[rhs_read_pos_0]);
    shared.rhs[rhs_write_pos_1] = FC::cast_from(rhs[rhs_read_pos_1]);
}