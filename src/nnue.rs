use crate::board::Board;
use crate::types::{Color, Piece, PieceKind};
use byteorder::{LittleEndian, ReadBytesExt};
use once_cell::sync::OnceCell;
use std::error::Error;
use std::fmt;
use std::io::{BufReader, Cursor, Read, Seek};

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

const FEATURE_TRANSFORMER_HALF_DIMENSIONS: usize = 256;
const SQUARE_NB: usize = 64;
const FT_INPUT_DIM: usize = 41024;
const HL1_INPUT_DIM: usize = 512;
const HL1_OUTPUT_DIM: usize = 32;
const HL2_OUTPUT_DIM: usize = 32;

pub struct Model {
    ft_weights: Vec<i16>,
    ft_biases: Vec<i16>,
    hl1_weights: Vec<i8>,
    hl1_biases: Vec<i32>,
    hl2_weights: Vec<i8>,
    hl2_biases: Vec<i32>,
    out_weights: Vec<i8>,
    out_bias: i32,
}

static MODEL: OnceCell<Model> = OnceCell::new();

#[derive(Debug)]
pub enum NnueError {
    IoError(std::io::Error),
    ValueError(String),
    AlreadyInitialized,
}

impl fmt::Display for NnueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NnueError::IoError(e) => write!(f, "I/O Error: {}", e),
            NnueError::ValueError(msg) => write!(f, "Value Error: {}", msg),
            NnueError::AlreadyInitialized => write!(f, "Model has already been initialized!"),
        }
    }
}

impl Error for NnueError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NnueError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for NnueError {
    fn from(e: std::io::Error) -> Self {
        NnueError::IoError(e)
    }
}

/// Initializes the NNUE model from the given file path.
pub fn init() -> Result<(), NnueError> {
    const NNUE_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/nn-9931db908a9b.nnue"));
    let mut reader = BufReader::new(Cursor::new(NNUE_BYTES));

    // Read headers and metadata
    let _version = reader.read_u32::<LittleEndian>()?;
    let _hash_value = reader.read_u32::<LittleEndian>()?;
    let desc_size = reader.read_u32::<LittleEndian>()? as usize;
    let mut desc_bytes = vec![0u8; desc_size];
    reader.read_exact(&mut desc_bytes)?;

    // Feature Transformer Weights and Biases
    let ft_header = reader.read_u32::<LittleEndian>()?;
    let expected_ft_hash = (0x5D69D5B9_u32 ^ 1) ^ (2 * FEATURE_TRANSFORMER_HALF_DIMENSIONS as u32);
    if ft_header != expected_ft_hash {
        return Err(NnueError::ValueError(
            "Feature transformer header does not match expected hash!".to_string(),
        ));
    }

    let mut ft_biases = vec![0i16; FEATURE_TRANSFORMER_HALF_DIMENSIONS];
    reader.read_i16_into::<LittleEndian>(&mut ft_biases)?;
    let ft_weights_count = FEATURE_TRANSFORMER_HALF_DIMENSIONS * FT_INPUT_DIM;
    let mut ft_weights = vec![0i16; ft_weights_count];
    reader.read_i16_into::<LittleEndian>(&mut ft_weights)?;

    // Layer 1 Weights and Biases
    let _l1_header = reader.read_u32::<LittleEndian>()?;
    let mut hl1_biases = vec![0i32; HL1_OUTPUT_DIM];
    reader.read_i32_into::<LittleEndian>(&mut hl1_biases)?;
    let hl1_weights_count = HL1_INPUT_DIM * HL1_OUTPUT_DIM;
    let mut hl1_weights = vec![0i8; hl1_weights_count];
    reader.read_i8_into(&mut hl1_weights)?;

    // Layer 2 Weights and Biases
    let mut hl2_biases = vec![0i32; HL2_OUTPUT_DIM];
    reader.read_i32_into::<LittleEndian>(&mut hl2_biases)?;
    let hl2_weights_count = HL2_OUTPUT_DIM * HL2_OUTPUT_DIM;
    let mut hl2_weights = vec![0i8; hl2_weights_count];
    reader.read_i8_into(&mut hl2_weights)?;

    // Output Layer Weights and Bias
    let out_bias = reader.read_i32::<LittleEndian>()?;
    let mut out_weights = vec![0i8; HL2_OUTPUT_DIM];
    reader.read_i8_into(&mut out_weights)?;

    let current_pos = reader.stream_position()?;
    let end_pos = reader.get_ref().get_ref().len() as u64;
    if end_pos - current_pos != 0 {
        return Err(NnueError::ValueError(
            "Did not read all parameters from NNUE file!".to_string(),
        ));
    }

    let model = Model {
        ft_weights,
        ft_biases,
        hl1_weights,
        hl1_biases,
        hl2_weights,
        hl2_biases,
        out_weights,
        out_bias,
    };

    MODEL
        .set(model)
        .map_err(|_| NnueError::AlreadyInitialized)?;
    Ok(())
}

/// Evaluates the board position using the loaded NNUE model.
pub fn evaluate(board: &Board) -> i32 {
    let model = MODEL
        .get()
        .expect("NNUE model not initialized! Call init() first.");

    let is_white_turn = board.turn == Color::White;

    // Get features from both points of view
    let (indices_us_array, count_us) = get_halfkp_indices(board, is_white_turn);
    let (indices_them_array, count_them) = get_halfkp_indices(board, !is_white_turn);

    let features_us = &indices_us_array[..count_us];
    let features_them = &indices_them_array[..count_them];

    // Apply feature transformer
    let ft_us =
        unsafe { feature_transformer_simd(features_us, &model.ft_weights, &model.ft_biases) };
    let ft_them =
        unsafe { feature_transformer_simd(features_them, &model.ft_weights, &model.ft_biases) };

    // Concatenate features for the first dense layer
    let mut concat_features = [0i32; HL1_INPUT_DIM];
    concat_features[..FEATURE_TRANSFORMER_HALF_DIMENSIONS].copy_from_slice(&ft_us);
    concat_features[FEATURE_TRANSFORMER_HALF_DIMENSIONS..].copy_from_slice(&ft_them);

    // Propagate through the network
    let hl1_out = dense_layer(
        &concat_features,
        &model.hl1_weights,
        &model.hl1_biases,
        HL1_INPUT_DIM,
        HL1_OUTPUT_DIM,
    );
    let hl2_out = dense_layer(
        &hl1_out,
        &model.hl2_weights,
        &model.hl2_biases,
        HL1_OUTPUT_DIM,
        HL2_OUTPUT_DIM,
    );
    let out_value = dense_output(&hl2_out, &model.out_weights, model.out_bias);

    // Convert final value to centipawns
    nn_value_to_centipawn(out_value)
}

/// Generates the list of active feature indices for one side.
#[inline]
fn get_halfkp_indices(board: &Board, is_white_pov: bool) -> ([usize; 32], usize) {
    let mut indices_array = [0; 32];
    let mut count = 0;

    let us = if is_white_pov {
        Color::White
    } else {
        Color::Black
    };
    let king_sq = board.king_square(us) as usize;
    let king_oriented = orient(is_white_pov, king_sq);

    for sq in 0..64 {
        let piece = board.piece_on[sq];
        if piece.is_empty() || piece.kind() == Some(PieceKind::King) {
            continue;
        }

        let piece_color = piece.color().unwrap();
        let idx = make_halfkp_index(is_white_pov, king_oriented, sq, piece, piece_color);

        if count < 32 {
            indices_array[count] = idx;
            count += 1;
        }
    }

    (indices_array, count)
}

#[inline]
fn make_halfkp_index(
    is_white_pov: bool,
    king_oriented: usize,
    sq: usize,
    piece: Piece,
    piece_color: Color,
) -> usize {
    let oriented_sq = orient(is_white_pov, sq);
    let piece_idx = piece_square_from_piece(piece.kind().unwrap(), piece_color, is_white_pov);
    oriented_sq + piece_idx + 641 * king_oriented
}

#[inline]
fn orient(is_white_pov: bool, sq: usize) -> usize {
    if is_white_pov { sq } else { sq ^ 56 }
}

/// This function maps a piece to its base index in the feature vector.
#[inline]
fn piece_square_from_piece(piece_kind: PieceKind, piece_color: Color, is_white_pov: bool) -> usize {
    let color_is_pov = (piece_color == Color::White) == is_white_pov;
    let color_offset = if color_is_pov { 0 } else { 1 };

    let piece_offset = match piece_kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    };

    (piece_offset * 2 + color_offset) * SQUARE_NB + 1
}

#[allow(dead_code)]
fn feature_transformer(
    indices: &[usize],
    ft_weights: &[i16],
    ft_biases: &[i16],
) -> [i32; FEATURE_TRANSFORMER_HALF_DIMENSIONS] {
    let mut out = [0i32; FEATURE_TRANSFORMER_HALF_DIMENSIONS];
    for i in 0..FEATURE_TRANSFORMER_HALF_DIMENSIONS {
        out[i] = ft_biases[i] as i32;
    }
    for &idx in indices {
        let base = idx * FEATURE_TRANSFORMER_HALF_DIMENSIONS;
        for i in 0..FEATURE_TRANSFORMER_HALF_DIMENSIONS {
            out[i] += ft_weights[base + i] as i32;
        }
    }
    for v in &mut out {
        *v = (*v).clamp(0, 127);
    }
    out
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
fn feature_transformer_simd(
    indices: &[usize],
    ft_weights: &[i16],
    ft_biases: &[i16],
) -> [i32; FEATURE_TRANSFORMER_HALF_DIMENSIONS] {
    let mut out = [0i32; FEATURE_TRANSFORMER_HALF_DIMENSIONS];
    let mut out_vecs: Vec<__m256i> = Vec::with_capacity(FEATURE_TRANSFORMER_HALF_DIMENSIONS / 8);

    // Initialize with biases
    let mut i = 0;
    while i < FEATURE_TRANSFORMER_HALF_DIMENSIONS {
        unsafe {
            let bias_ptr = ft_biases.as_ptr().add(i);
            let bias_i16 = _mm_loadu_si128(bias_ptr as *const __m128i);
            out_vecs.push(_mm256_cvtepi16_epi32(bias_i16));
        }
        i += 8;
    }

    // Accumulate weights
    for &idx in indices {
        let base = idx * FEATURE_TRANSFORMER_HALF_DIMENSIONS;
        let mut i = 0;
        while i < FEATURE_TRANSFORMER_HALF_DIMENSIONS {
            let out_vec = out_vecs[i / 8];
            unsafe {
                let wt_ptr = ft_weights.as_ptr().add(base + i);
                let wt_i16 = _mm_loadu_si128(wt_ptr as *const __m128i);
                let wt_vec = _mm256_cvtepi16_epi32(wt_i16);
                let sum_vec = _mm256_add_epi32(out_vec, wt_vec);
                out_vecs[i / 8] = sum_vec;
            }
            i += 8;
        }
    }

    // Clamp and store
    let zero = _mm256_setzero_si256();
    let max = _mm256_set1_epi32(127);
    let mut i = 0;
    while i < FEATURE_TRANSFORMER_HALF_DIMENSIONS {
        let v = out_vecs[i / 8];
        let clamped = _mm256_min_epi32(_mm256_max_epi32(v, zero), max);
        unsafe {
            _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, clamped);
        }
        i += 8;
    }
    out
}

#[cfg(not(target_arch = "x86_64"))]
fn feature_transformer_simd(
    indices: &[usize],
    ft_weights: &[i16],
    ft_biases: &[i16],
) -> [i32; FEATURE_TRANSFORMER_HALF_DIMENSIONS] {
    feature_transformer(indices, ft_weights, ft_biases)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
fn dot_product_avx2(input: &[i32], weights: &[i8]) -> i32 {
    let len = input.len();
    let mut i = 0;
    let mut acc = _mm256_setzero_si256();

    while i + 16 <= len {
        unsafe {
            let in_vec1 = _mm256_loadu_si256(input.as_ptr().add(i) as *const __m256i);
            let wt_chunk1 = _mm_loadl_epi64(weights.as_ptr().add(i) as *const __m128i);
            let wt_vec1 = _mm256_cvtepi8_epi32(wt_chunk1);

            let in_vec2 = _mm256_loadu_si256(input.as_ptr().add(i + 8) as *const __m256i);
            let wt_chunk2 = _mm_loadl_epi64(weights.as_ptr().add(i + 8) as *const __m128i);
            let wt_vec2 = _mm256_cvtepi8_epi32(wt_chunk2);

            let prod1 = _mm256_madd_epi16(
                _mm256_packs_epi32(in_vec1, in_vec2),
                _mm256_packs_epi32(wt_vec1, wt_vec2),
            );
            acc = _mm256_add_epi32(acc, prod1);
        }
        i += 16;
    }

    let mut acc_arr = [0i32; 8];
    unsafe {
        _mm256_storeu_si256(acc_arr.as_mut_ptr() as *mut __m256i, acc);
    }
    let mut sum = acc_arr.iter().sum();

    while i < len {
        sum += input[i] * (weights[i] as i32);
        i += 1;
    }
    sum
}

#[cfg(not(target_arch = "x86_64"))]
fn dot_product_avx2(input: &[i32], weights: &[i8]) -> i32 {
    input
        .iter()
        .zip(weights.iter())
        .map(|(&x, &w)| x * (w as i32))
        .sum()
}

#[inline]
fn dense_layer(
    input: &[i32],
    weights: &[i8],
    biases: &[i32],
    in_dim: usize,
    out_dim: usize,
) -> [i32; HL1_OUTPUT_DIM] {
    let mut out = [0i32; HL1_OUTPUT_DIM];
    for j in 0..out_dim {
        let weight_slice = &weights[j * in_dim..(j + 1) * in_dim];
        let sum = biases[j] + unsafe { dot_product_avx2(input, weight_slice) };
        out[j] = nnue_relu(sum);
    }
    out
}

#[inline]
fn dense_output(input: &[i32], weights: &[i8], bias: i32) -> i32 {
    bias + input
        .iter()
        .zip(weights.iter())
        .map(|(&x, &w)| x * (w as i32))
        .sum::<i32>()
}

#[inline]
fn nnue_relu(x: i32) -> i32 {
    if x < 0 {
        0
    } else {
        let y = x / 64;
        if y > 127 { 127 } else { y }
    }
}

#[inline]
fn floor_div(a: i32, b: i32) -> i32 {
    let mut d = a / b;
    if (a > 0) != (b > 0) && a % b != 0 {
        d -= 1;
    }
    d
}

#[inline]
fn nn_value_to_centipawn(nn_value: i32) -> i32 {
    let v = floor_div(nn_value, 8);
    floor_div(v * 100, 208)
}
