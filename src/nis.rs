use std::num::NonZeroU32;

use bevy::{
    ecs::query::QueryItem,
    prelude::{Camera, Component, Res, ResMut},
    render::{
        extract_component::ExtractComponent,
        render_resource::{
            Extent3d, ImageDataLayout, ShaderType, TextureDescriptor, TextureDimension,
            TextureFormat, TextureUsages, TextureView,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::TextureCache,
    },
};
use bytemuck::cast_ref;

use crate::HikariConfig;

// TODO rename ot NISConfigUniform
#[derive(Debug, Default, Clone, Copy, Component, ShaderType)]
pub struct NISConfig {
    pub k_detect_ratio: f32,
    pub k_detect_thres: f32,
    pub k_min_contrast_ratio: f32,
    pub k_ratio_norm: f32,

    pub k_contrast_boost: f32,
    pub k_eps: f32,
    pub k_sharp_start_y: f32,
    pub k_sharp_scale_y: f32,

    pub k_sharp_strength_min: f32,
    pub k_sharp_strength_scale: f32,
    pub k_sharp_limit_min: f32,
    pub k_sharp_limit_scale: f32,

    pub k_scale_x: f32,
    pub k_scale_y: f32,
    pub k_dst_norm_x: f32,
    pub k_dst_norm_y: f32,

    pub k_src_norm_x: f32,
    pub k_src_norm_y: f32,

    pub k_input_viewport_origin_x: u32,
    pub k_input_viewport_origin_y: u32,
    pub k_input_viewport_width: u32,
    pub k_input_viewport_height: u32,

    pub k_output_viewport_origin_x: u32,
    pub k_output_viewport_origin_y: u32,
    pub k_output_viewport_width: u32,
    pub k_output_viewport_height: u32,

    pub reserved0: f32,
    pub reserved1: f32,
}

#[derive(Debug, Default, Clone, Copy, Component, ShaderType)]
pub struct NisScaleConfig {
    pub config: NISConfig,
}

impl NisScaleConfig {
    fn update_config(
        &mut self,
        sharpness: f32,
        input_viewport_origin_x: u32,
        input_viewport_origin_y: u32,
        input_viewport_width: u32,
        input_viewport_height: u32,
        input_texture_width: u32,
        input_texture_height: u32,
        output_viewport_origin_x: u32,
        output_viewport_origin_y: u32,
        output_viewport_width: u32,
        output_viewport_height: u32,
        output_texture_width: u32,
        output_texture_height: u32,
        hdr_mode: NISHDRMode,
    ) -> bool {
        nv_scaler_update_config(
            &mut self.config,
            sharpness,
            input_viewport_origin_x,
            input_viewport_origin_y,
            input_viewport_width,
            input_viewport_height,
            input_texture_width,
            input_texture_height,
            output_viewport_origin_x,
            output_viewport_origin_y,
            output_viewport_width,
            output_viewport_height,
            output_texture_width,
            output_texture_height,
            hdr_mode,
        )
    }
}

#[derive(Debug, Default, Clone, Copy, Component, ShaderType)]
pub struct NisSharpenConfig {
    pub config: NISConfig,
}

impl NisSharpenConfig {
    fn update_config(
        &mut self,
        sharpness: f32,
        input_viewport_origin_x: u32,
        input_viewport_origin_y: u32,
        input_viewport_width: u32,
        input_viewport_height: u32,
        input_texture_width: u32,
        input_texture_height: u32,
        output_viewport_origin_x: u32,
        output_viewport_origin_y: u32,
        hdr_mode: NISHDRMode,
    ) -> bool {
        nv_sharpen_update_config(
            &mut self.config,
            sharpness,
            input_viewport_origin_x,
            input_viewport_origin_y,
            input_viewport_width,
            input_viewport_height,
            input_texture_width,
            input_texture_height,
            output_viewport_origin_x,
            output_viewport_origin_y,
            hdr_mode,
        )
    }
}

impl ExtractComponent for NisScaleConfig {
    type Query = (&'static Camera, &'static HikariConfig);
    type Filter = ();

    fn extract_component((camera, config): QueryItem<Self::Query>) -> Self {
        let size = camera.physical_target_size().unwrap_or_default();
        let scale = 1.0 / config.upscale_ratio();
        let before_upscale_size_x = (size.x as f32 * scale).ceil() as u32;
        let before_upscale_size_y = (size.y as f32 * scale).ceil() as u32;

        let mut nis_config: NisScaleConfig = NisScaleConfig::default();
        let succ = nis_config.update_config(
            0.25,
            0,
            0,
            before_upscale_size_x,
            before_upscale_size_y,
            before_upscale_size_x,
            before_upscale_size_y,
            0,
            0,
            size.x,
            size.y,
            size.x,
            size.y,
            NISHDRMode::None,
        );

        //println!("in_x = {:?}", before_upscale_size_x);
        //println!("in_y = {:?}", before_upscale_size_y);
        //println!("out_x = {:?}", size.x);
        //println!("out_y = {:?}", size.y);

        nis_config
    }
}

impl ExtractComponent for NisSharpenConfig {
    type Query = &'static Camera;
    type Filter = ();

    fn extract_component(camera: QueryItem<Self::Query>) -> Self {
        let size = camera.physical_target_size().unwrap_or_default();

        let mut nis_config: NisSharpenConfig = NisSharpenConfig::default();
        let succ = nis_config.update_config(
            0.25,
            0,
            0,
            size.x,
            size.y,
            size.x,
            size.y,
            0,
            0,
            NISHDRMode::None,
        );

        //println!("succ sh = {:?}", succ);

        nis_config
    }
}

pub enum NISHDRMode {
    None = 0,
    Linear = 1,
    PQ = 2,
}

fn nv_scaler_update_config(
    config: &mut NISConfig,
    mut sharpness: f32,
    input_viewport_origin_x: u32,
    input_viewport_origin_y: u32,
    input_viewport_width: u32,
    input_viewport_height: u32,
    input_texture_width: u32,
    input_texture_height: u32,
    output_viewport_origin_x: u32,
    output_viewport_origin_y: u32,
    output_viewport_width: u32,
    output_viewport_height: u32,
    output_texture_width: u32,
    output_texture_height: u32,
    hdr_mode: NISHDRMode,
) -> bool {
    // adjust params based on value from sharpness slider
    sharpness = f32::max(f32::min(1.0, sharpness), 0.0);
    let sharpen_slider = sharpness - 0.5; // Map 0 to 1 to -0.5 to +0.5

    // Different range for 0 to 50% vs 50% to 100%
    // The idea is to make sure sharpness of 0% map to no-sharpening,
    // while also ensuring that sharpness of 100% doesn't cause too much over-sharpening.
    let mut max_scale: f32 = 1.75;
    let mut min_scale: f32 = 1.0;
    let mut limit_scale: f32 = 1.0;

    if sharpen_slider >= 0.0 {
        max_scale = 1.25;
        min_scale = 1.25;
        limit_scale = 1.25;
    }

    let k_detect_ratio: f32 = 2.0 * 1127.0 / 1024.0;

    // params for sdr
    let mut k_detect_thres: f32 = 64.0 / 1024.0;
    let mut k_min_contrast_ratio: f32 = 2.0;
    let mut k_max_contrast_ratio: f32 = 10.0;

    let mut k_sharp_start_y: f32 = 0.45;
    let mut k_sharp_end_y: f32 = 0.9;
    let mut k_sharp_strength_min: f32 = f32::max(0.0, 0.4 + sharpen_slider * min_scale * 1.2);
    let mut k_sharp_strength_max: f32 = 1.6 + sharpen_slider * max_scale * 1.8;
    let mut k_sharp_limit_min: f32 = f32::max(0.1, 0.14 + sharpen_slider * limit_scale * 0.32);
    let mut k_sharp_limit_max: f32 = 0.5 + sharpen_slider * limit_scale * 0.6;

    match hdr_mode {
        NISHDRMode::Linear | NISHDRMode::PQ => {
            k_detect_thres = 32.0 / 1024.0;

            k_min_contrast_ratio = 1.5;
            k_max_contrast_ratio = 5.0;

            k_sharp_strength_min = f32::max(0.0, 0.4 + sharpen_slider * min_scale * 1.1);
            k_sharp_strength_max = 2.2 + sharpen_slider * max_scale * 1.8;
            k_sharp_limit_min = f32::max(0.06, 0.10 + sharpen_slider * limit_scale * 0.28);
            k_sharp_limit_max = 0.6 + sharpen_slider * limit_scale * 0.6;

            match hdr_mode {
                NISHDRMode::PQ => {
                    k_sharp_start_y = 0.35;
                    k_sharp_end_y = 0.55;
                }
                _ => {
                    k_sharp_start_y = 0.3;
                    k_sharp_end_y = 0.5;
                }
            }
        }
        _ => (),
    }

    let k_ratio_norm: f32 = 1.0 / (k_max_contrast_ratio - k_min_contrast_ratio);
    let k_sharp_scale_y: f32 = 1.0 / (k_sharp_end_y - k_sharp_start_y);
    let k_sharp_strength_scale: f32 = k_sharp_strength_max - k_sharp_strength_min;
    let k_sharp_limit_scale: f32 = k_sharp_limit_max - k_sharp_limit_min;

    config.k_input_viewport_width = if input_viewport_width == 0 {
        input_texture_width
    } else {
        input_viewport_width
    };
    config.k_input_viewport_height = if input_viewport_height == 0 {
        input_texture_height
    } else {
        input_viewport_height
    };
    config.k_output_viewport_width = if output_viewport_width == 0 {
        output_texture_width
    } else {
        output_viewport_width
    };
    config.k_output_viewport_height = if output_viewport_height == 0 {
        output_texture_height
    } else {
        output_viewport_height
    };

    if config.k_input_viewport_width == 0
        || config.k_input_viewport_height == 0
        || config.k_output_viewport_width == 0
        || config.k_output_viewport_height == 0
    {
        return false;
    }

    config.k_input_viewport_origin_x = input_viewport_origin_x;
    config.k_input_viewport_origin_y = input_viewport_origin_y;
    config.k_output_viewport_origin_x = output_viewport_origin_x;
    config.k_output_viewport_origin_y = output_viewport_origin_y;

    config.k_src_norm_x = 1.0 / input_texture_width as f32;
    config.k_src_norm_y = 1.0 / input_texture_height as f32;
    config.k_dst_norm_x = 1.0 / output_texture_width as f32;
    config.k_dst_norm_y = 1.0 / output_texture_height as f32;
    config.k_scale_x = config.k_input_viewport_width as f32 / config.k_output_viewport_width as f32;
    config.k_scale_y =
        config.k_input_viewport_height as f32 / config.k_output_viewport_height as f32;
    config.k_detect_ratio = k_detect_ratio;
    config.k_detect_thres = k_detect_thres;
    config.k_min_contrast_ratio = k_min_contrast_ratio;
    config.k_ratio_norm = k_ratio_norm;
    config.k_contrast_boost = 1.0;
    config.k_eps = 1.0 / 255.0;
    config.k_sharp_start_y = k_sharp_start_y;
    config.k_sharp_scale_y = k_sharp_scale_y;
    config.k_sharp_strength_min = k_sharp_strength_min;
    config.k_sharp_strength_scale = k_sharp_strength_scale;
    config.k_sharp_limit_min = k_sharp_limit_min;
    config.k_sharp_limit_scale = k_sharp_limit_scale;

    if config.k_scale_x < 0.5
        || config.k_scale_x > 1.0
        || config.k_scale_y < 0.5
        || config.k_scale_y > 1.0
    {
        return false;
    }

    true
}

fn nv_sharpen_update_config(
    config: &mut NISConfig,
    sharpness: f32,
    input_viewport_origin_x: u32,
    input_viewport_origin_y: u32,
    input_viewport_width: u32,
    input_viewport_height: u32,
    input_texture_width: u32,
    input_texture_height: u32,
    output_viewport_origin_x: u32,
    output_viewport_origin_y: u32,
    hdr_mode: NISHDRMode,
) -> bool {
    nv_scaler_update_config(
        config,
        sharpness,
        input_viewport_origin_x,
        input_viewport_origin_y,
        input_viewport_width,
        input_viewport_height,
        input_texture_width,
        input_texture_height,
        output_viewport_origin_x,
        output_viewport_origin_y,
        input_viewport_width,
        input_viewport_height,
        input_texture_width,
        input_texture_height,
        hdr_mode,
    )
}

pub const K_PHASE_COUNT: usize = 64;
pub const K_FILTER_SIZE: usize = 8;
pub const K_ROW_PITCH: usize = K_FILTER_SIZE * 4;
pub const K_IMAGE_SIZE: usize = K_PHASE_COUNT * K_ROW_PITCH;

pub const COEF_SCALE: &'static [[f32; K_FILTER_SIZE]; K_PHASE_COUNT] = &[
    [0.0, 0.0, 1.0000, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0029, -0.0127, 1.0000, 0.0132, -0.0034, 0.0, 0.0, 0.0],
    [0.0063, -0.0249, 0.9985, 0.0269, -0.0068, 0.0, 0.0, 0.0],
    [0.0088, -0.0361, 0.9956, 0.0415, -0.0103, 0.0005, 0.0, 0.0],
    [0.0117, -0.0474, 0.9932, 0.0562, -0.0142, 0.0005, 0.0, 0.0],
    [0.0142, -0.0576, 0.9897, 0.0713, -0.0181, 0.0005, 0.0, 0.0],
    [0.0166, -0.0674, 0.9844, 0.0874, -0.0220, 0.0010, 0.0, 0.0],
    [0.0186, -0.0762, 0.9785, 0.1040, -0.0264, 0.0015, 0.0, 0.0],
    [0.0205, -0.0850, 0.9727, 0.1206, -0.0308, 0.0020, 0.0, 0.0],
    [0.0225, -0.0928, 0.9648, 0.1382, -0.0352, 0.0024, 0.0, 0.0],
    [0.0239, -0.1006, 0.9575, 0.1558, -0.0396, 0.0029, 0.0, 0.0],
    [0.0254, -0.1074, 0.9487, 0.1738, -0.0439, 0.0034, 0.0, 0.0],
    [0.0264, -0.1138, 0.9390, 0.1929, -0.0488, 0.0044, 0.0, 0.0],
    [0.0278, -0.1191, 0.9282, 0.2119, -0.0537, 0.0049, 0.0, 0.0],
    [0.0288, -0.1245, 0.9170, 0.2310, -0.0581, 0.0059, 0.0, 0.0],
    [0.0293, -0.1294, 0.9058, 0.2510, -0.0630, 0.0063, 0.0, 0.0],
    [0.0303, -0.1333, 0.8926, 0.2710, -0.0679, 0.0073, 0.0, 0.0],
    [0.0308, -0.1367, 0.8789, 0.2915, -0.0728, 0.0083, 0.0, 0.0],
    [0.0308, -0.1401, 0.8657, 0.3120, -0.0776, 0.0093, 0.0, 0.0],
    [0.0313, -0.1426, 0.8506, 0.3330, -0.0825, 0.0103, 0.0, 0.0],
    [0.0313, -0.1445, 0.8354, 0.3540, -0.0874, 0.0112, 0.0, 0.0],
    [0.0313, -0.1460, 0.8193, 0.3755, -0.0923, 0.0122, 0.0, 0.0],
    [0.0313, -0.1470, 0.8022, 0.3965, -0.0967, 0.0137, 0.0, 0.0],
    [0.0308, -0.1479, 0.7856, 0.4185, -0.1016, 0.0146, 0.0, 0.0],
    [0.0303, -0.1479, 0.7681, 0.4399, -0.1060, 0.0156, 0.0, 0.0],
    [0.0298, -0.1479, 0.7505, 0.4614, -0.1104, 0.0166, 0.0, 0.0],
    [0.0293, -0.1470, 0.7314, 0.4829, -0.1147, 0.0181, 0.0, 0.0],
    [0.0288, -0.1460, 0.7119, 0.5049, -0.1187, 0.0190, 0.0, 0.0],
    [0.0278, -0.1445, 0.6929, 0.5264, -0.1226, 0.0200, 0.0, 0.0],
    [0.0273, -0.1431, 0.6724, 0.5479, -0.1260, 0.0215, 0.0, 0.0],
    [0.0264, -0.1411, 0.6528, 0.5693, -0.1299, 0.0225, 0.0, 0.0],
    [0.0254, -0.1387, 0.6323, 0.5903, -0.1328, 0.0234, 0.0, 0.0],
    [0.0244, -0.1357, 0.6113, 0.6113, -0.1357, 0.0244, 0.0, 0.0],
    [0.0234, -0.1328, 0.5903, 0.6323, -0.1387, 0.0254, 0.0, 0.0],
    [0.0225, -0.1299, 0.5693, 0.6528, -0.1411, 0.0264, 0.0, 0.0],
    [0.0215, -0.1260, 0.5479, 0.6724, -0.1431, 0.0273, 0.0, 0.0],
    [0.0200, -0.1226, 0.5264, 0.6929, -0.1445, 0.0278, 0.0, 0.0],
    [0.0190, -0.1187, 0.5049, 0.7119, -0.1460, 0.0288, 0.0, 0.0],
    [0.0181, -0.1147, 0.4829, 0.7314, -0.1470, 0.0293, 0.0, 0.0],
    [0.0166, -0.1104, 0.4614, 0.7505, -0.1479, 0.0298, 0.0, 0.0],
    [0.0156, -0.1060, 0.4399, 0.7681, -0.1479, 0.0303, 0.0, 0.0],
    [0.0146, -0.1016, 0.4185, 0.7856, -0.1479, 0.0308, 0.0, 0.0],
    [0.0137, -0.0967, 0.3965, 0.8022, -0.1470, 0.0313, 0.0, 0.0],
    [0.0122, -0.0923, 0.3755, 0.8193, -0.1460, 0.0313, 0.0, 0.0],
    [0.0112, -0.0874, 0.3540, 0.8354, -0.1445, 0.0313, 0.0, 0.0],
    [0.0103, -0.0825, 0.3330, 0.8506, -0.1426, 0.0313, 0.0, 0.0],
    [0.0093, -0.0776, 0.3120, 0.8657, -0.1401, 0.0308, 0.0, 0.0],
    [0.0083, -0.0728, 0.2915, 0.8789, -0.1367, 0.0308, 0.0, 0.0],
    [0.0073, -0.0679, 0.2710, 0.8926, -0.1333, 0.0303, 0.0, 0.0],
    [0.0063, -0.0630, 0.2510, 0.9058, -0.1294, 0.0293, 0.0, 0.0],
    [0.0059, -0.0581, 0.2310, 0.9170, -0.1245, 0.0288, 0.0, 0.0],
    [0.0049, -0.0537, 0.2119, 0.9282, -0.1191, 0.0278, 0.0, 0.0],
    [0.0044, -0.0488, 0.1929, 0.9390, -0.1138, 0.0264, 0.0, 0.0],
    [0.0034, -0.0439, 0.1738, 0.9487, -0.1074, 0.0254, 0.0, 0.0],
    [0.0029, -0.0396, 0.1558, 0.9575, -0.1006, 0.0239, 0.0, 0.0],
    [0.0024, -0.0352, 0.1382, 0.9648, -0.0928, 0.0225, 0.0, 0.0],
    [0.0020, -0.0308, 0.1206, 0.9727, -0.0850, 0.0205, 0.0, 0.0],
    [0.0015, -0.0264, 0.1040, 0.9785, -0.0762, 0.0186, 0.0, 0.0],
    [0.0010, -0.0220, 0.0874, 0.9844, -0.0674, 0.0166, 0.0, 0.0],
    [0.0005, -0.0181, 0.0713, 0.9897, -0.0576, 0.0142, 0.0, 0.0],
    [0.0005, -0.0142, 0.0562, 0.9932, -0.0474, 0.0117, 0.0, 0.0],
    [0.0005, -0.0103, 0.0415, 0.9956, -0.0361, 0.0088, 0.0, 0.0],
    [0.0, -0.0068, 0.0269, 0.9985, -0.0249, 0.0063, 0.0, 0.0],
    [0.0, -0.0034, 0.0132, 1.0000, -0.0127, 0.0029, 0.0, 0.0],
];

pub const COEF_USM: &'static [[f32; K_FILTER_SIZE]; K_PHASE_COUNT] = &[
    [0.0, -0.6001, 1.2002, -0.6001, 0.0, 0.0, 0.0, 0.0],
    [0.0029, -0.6084, 1.1987, -0.5903, -0.0029, 0.0, 0.0, 0.0],
    [0.0049, -0.6147, 1.1958, -0.5791, -0.0068, 0.0005, 0.0, 0.0],
    [0.0073, -0.6196, 1.1890, -0.5659, -0.0103, 0.0, 0.0, 0.0],
    [0.0093, -0.6235, 1.1802, -0.5513, -0.0151, 0.0, 0.0, 0.0],
    [0.0112, -0.6265, 1.1699, -0.5352, -0.0195, 0.0005, 0.0, 0.0],
    [0.0122, -0.6270, 1.1582, -0.5181, -0.0259, 0.0005, 0.0, 0.0],
    [0.0142, -0.6284, 1.1455, -0.5005, -0.0317, 0.0005, 0.0, 0.0],
    [0.0156, -0.6265, 1.1274, -0.4790, -0.0386, 0.0005, 0.0, 0.0],
    [0.0166, -0.6235, 1.1089, -0.4570, -0.0454, 0.0010, 0.0, 0.0],
    [0.0176, -0.6187, 1.0879, -0.4346, -0.0532, 0.0010, 0.0, 0.0],
    [0.0181, -0.6138, 1.0659, -0.4102, -0.0615, 0.0015, 0.0, 0.0],
    [0.0190, -0.6069, 1.0405, -0.3843, -0.0698, 0.0015, 0.0, 0.0],
    [0.0195, -0.6006, 1.0161, -0.3574, -0.0796, 0.0020, 0.0, 0.0],
    [0.0200, -0.5928, 0.9893, -0.3286, -0.0898, 0.0024, 0.0, 0.0],
    [0.0200, -0.5820, 0.9580, -0.2988, -0.1001, 0.0029, 0.0, 0.0],
    [0.0200, -0.5728, 0.9292, -0.2690, -0.1104, 0.0034, 0.0, 0.0],
    [0.0200, -0.5620, 0.8975, -0.2368, -0.1226, 0.0039, 0.0, 0.0],
    [0.0205, -0.5498, 0.8643, -0.2046, -0.1343, 0.0044, 0.0, 0.0],
    [0.0200, -0.5371, 0.8301, -0.1709, -0.1465, 0.0049, 0.0, 0.0],
    [0.0195, -0.5239, 0.7944, -0.1367, -0.1587, 0.0054, 0.0, 0.0],
    [0.0195, -0.5107, 0.7598, -0.1021, -0.1724, 0.0059, 0.0, 0.0],
    [0.0190, -0.4966, 0.7231, -0.0649, -0.1865, 0.0063, 0.0, 0.0],
    [0.0186, -0.4819, 0.6846, -0.0288, -0.1997, 0.0068, 0.0, 0.0],
    [0.0186, -0.4668, 0.6460, 0.0093, -0.2144, 0.0073, 0.0, 0.0],
    [0.0176, -0.4507, 0.6055, 0.0479, -0.2290, 0.0083, 0.0, 0.0],
    [0.0171, -0.4370, 0.5693, 0.0859, -0.2446, 0.0088, 0.0, 0.0],
    [0.0161, -0.4199, 0.5283, 0.1255, -0.2598, 0.0098, 0.0, 0.0],
    [0.0161, -0.4048, 0.4883, 0.1655, -0.2754, 0.0103, 0.0, 0.0],
    [0.0151, -0.3887, 0.4497, 0.2041, -0.2910, 0.0107, 0.0, 0.0],
    [0.0142, -0.3711, 0.4072, 0.2446, -0.3066, 0.0117, 0.0, 0.0],
    [0.0137, -0.3555, 0.3672, 0.2852, -0.3228, 0.0122, 0.0, 0.0],
    [0.0132, -0.3394, 0.3262, 0.3262, -0.3394, 0.0132, 0.0, 0.0],
    [0.0122, -0.3228, 0.2852, 0.3672, -0.3555, 0.0137, 0.0, 0.0],
    [0.0117, -0.3066, 0.2446, 0.4072, -0.3711, 0.0142, 0.0, 0.0],
    [0.0107, -0.2910, 0.2041, 0.4497, -0.3887, 0.0151, 0.0, 0.0],
    [0.0103, -0.2754, 0.1655, 0.4883, -0.4048, 0.0161, 0.0, 0.0],
    [0.0098, -0.2598, 0.1255, 0.5283, -0.4199, 0.0161, 0.0, 0.0],
    [0.0088, -0.2446, 0.0859, 0.5693, -0.4370, 0.0171, 0.0, 0.0],
    [0.0083, -0.2290, 0.0479, 0.6055, -0.4507, 0.0176, 0.0, 0.0],
    [0.0073, -0.2144, 0.0093, 0.6460, -0.4668, 0.0186, 0.0, 0.0],
    [0.0068, -0.1997, -0.0288, 0.6846, -0.4819, 0.0186, 0.0, 0.0],
    [0.0063, -0.1865, -0.0649, 0.7231, -0.4966, 0.0190, 0.0, 0.0],
    [0.0059, -0.1724, -0.1021, 0.7598, -0.5107, 0.0195, 0.0, 0.0],
    [0.0054, -0.1587, -0.1367, 0.7944, -0.5239, 0.0195, 0.0, 0.0],
    [0.0049, -0.1465, -0.1709, 0.8301, -0.5371, 0.0200, 0.0, 0.0],
    [0.0044, -0.1343, -0.2046, 0.8643, -0.5498, 0.0205, 0.0, 0.0],
    [0.0039, -0.1226, -0.2368, 0.8975, -0.5620, 0.0200, 0.0, 0.0],
    [0.0034, -0.1104, -0.2690, 0.9292, -0.5728, 0.0200, 0.0, 0.0],
    [0.0029, -0.1001, -0.2988, 0.9580, -0.5820, 0.0200, 0.0, 0.0],
    [0.0024, -0.0898, -0.3286, 0.9893, -0.5928, 0.0200, 0.0, 0.0],
    [0.0020, -0.0796, -0.3574, 1.0161, -0.6006, 0.0195, 0.0, 0.0],
    [0.0015, -0.0698, -0.3843, 1.0405, -0.6069, 0.0190, 0.0, 0.0],
    [0.0015, -0.0615, -0.4102, 1.0659, -0.6138, 0.0181, 0.0, 0.0],
    [0.0010, -0.0532, -0.4346, 1.0879, -0.6187, 0.0176, 0.0, 0.0],
    [0.0010, -0.0454, -0.4570, 1.1089, -0.6235, 0.0166, 0.0, 0.0],
    [0.0005, -0.0386, -0.4790, 1.1274, -0.6265, 0.0156, 0.0, 0.0],
    [0.0005, -0.0317, -0.5005, 1.1455, -0.6284, 0.0142, 0.0, 0.0],
    [0.0005, -0.0259, -0.5181, 1.1582, -0.6270, 0.0122, 0.0, 0.0],
    [0.0005, -0.0195, -0.5352, 1.1699, -0.6265, 0.0112, 0.0, 0.0],
    [0.0, -0.0151, -0.5513, 1.1802, -0.6235, 0.0093, 0.0, 0.0],
    [0.0, -0.0103, -0.5659, 1.1890, -0.6196, 0.0073, 0.0, 0.0],
    [0.0005, -0.0068, -0.5791, 1.1958, -0.6147, 0.0049, 0.0, 0.0],
    [0.0, -0.0029, -0.5903, 1.1987, -0.6084, 0.0029, 0.0, 0.0],
];

pub const COEF_SCALE_FP16: &'static [[u16; K_FILTER_SIZE]; K_PHASE_COUNT] = &[
    [0, 0, 15360, 0, 0, 0, 0, 0],
    [6640, 41601, 15360, 8898, 39671, 0, 0, 0],
    [7796, 42592, 15357, 9955, 40695, 0, 0, 0],
    [8321, 43167, 15351, 10576, 41286, 4121, 0, 0],
    [8702, 43537, 15346, 11058, 41797, 4121, 0, 0],
    [9029, 43871, 15339, 11408, 42146, 4121, 0, 0],
    [9280, 44112, 15328, 11672, 42402, 5145, 0, 0],
    [9411, 44256, 15316, 11944, 42690, 5669, 0, 0],
    [9535, 44401, 15304, 12216, 42979, 6169, 0, 0],
    [9667, 44528, 15288, 12396, 43137, 6378, 0, 0],
    [9758, 44656, 15273, 12540, 43282, 6640, 0, 0],
    [9857, 44768, 15255, 12688, 43423, 6903, 0, 0],
    [9922, 44872, 15235, 12844, 43583, 7297, 0, 0],
    [10014, 44959, 15213, 13000, 43744, 7429, 0, 0],
    [10079, 45048, 15190, 13156, 43888, 7691, 0, 0],
    [10112, 45092, 15167, 13316, 44040, 7796, 0, 0],
    [10178, 45124, 15140, 13398, 44120, 8058, 0, 0],
    [10211, 45152, 15112, 13482, 44201, 8256, 0, 0],
    [10211, 45180, 15085, 13566, 44279, 8387, 0, 0],
    [10242, 45200, 15054, 13652, 44360, 8518, 0, 0],
    [10242, 45216, 15023, 13738, 44440, 8636, 0, 0],
    [10242, 45228, 14990, 13826, 44520, 8767, 0, 0],
    [10242, 45236, 14955, 13912, 44592, 8964, 0, 0],
    [10211, 45244, 14921, 14002, 44673, 9082, 0, 0],
    [10178, 45244, 14885, 14090, 44745, 9213, 0, 0],
    [10145, 45244, 14849, 14178, 44817, 9280, 0, 0],
    [10112, 45236, 14810, 14266, 44887, 9378, 0, 0],
    [10079, 45228, 14770, 14346, 44953, 9437, 0, 0],
    [10014, 45216, 14731, 14390, 45017, 9503, 0, 0],
    [9981, 45204, 14689, 14434, 45064, 9601, 0, 0],
    [9922, 45188, 14649, 14478, 45096, 9667, 0, 0],
    [9857, 45168, 14607, 14521, 45120, 9726, 0, 0],
    [9791, 45144, 14564, 14564, 45144, 9791, 0, 0],
    [9726, 45120, 14521, 14607, 45168, 9857, 0, 0],
    [9667, 45096, 14478, 14649, 45188, 9922, 0, 0],
    [9601, 45064, 14434, 14689, 45204, 9981, 0, 0],
    [9503, 45017, 14390, 14731, 45216, 10014, 0, 0],
    [9437, 44953, 14346, 14770, 45228, 10079, 0, 0],
    [9378, 44887, 14266, 14810, 45236, 10112, 0, 0],
    [9280, 44817, 14178, 14849, 45244, 10145, 0, 0],
    [9213, 44745, 14090, 14885, 45244, 10178, 0, 0],
    [9082, 44673, 14002, 14921, 45244, 10211, 0, 0],
    [8964, 44592, 13912, 14955, 45236, 10242, 0, 0],
    [8767, 44520, 13826, 14990, 45228, 10242, 0, 0],
    [8636, 44440, 13738, 15023, 45216, 10242, 0, 0],
    [8518, 44360, 13652, 15054, 45200, 10242, 0, 0],
    [8387, 44279, 13566, 15085, 45180, 10211, 0, 0],
    [8256, 44201, 13482, 15112, 45152, 10211, 0, 0],
    [8058, 44120, 13398, 15140, 45124, 10178, 0, 0],
    [7796, 44040, 13316, 15167, 45092, 10112, 0, 0],
    [7691, 43888, 13156, 15190, 45048, 10079, 0, 0],
    [7429, 43744, 13000, 15213, 44959, 10014, 0, 0],
    [7297, 43583, 12844, 15235, 44872, 9922, 0, 0],
    [6903, 43423, 12688, 15255, 44768, 9857, 0, 0],
    [6640, 43282, 12540, 15273, 44656, 9758, 0, 0],
    [6378, 43137, 12396, 15288, 44528, 9667, 0, 0],
    [6169, 42979, 12216, 15304, 44401, 9535, 0, 0],
    [5669, 42690, 11944, 15316, 44256, 9411, 0, 0],
    [5145, 42402, 11672, 15328, 44112, 9280, 0, 0],
    [4121, 42146, 11408, 15339, 43871, 9029, 0, 0],
    [4121, 41797, 11058, 15346, 43537, 8702, 0, 0],
    [4121, 41286, 10576, 15351, 43167, 8321, 0, 0],
    [0, 40695, 9955, 15357, 42592, 7796, 0, 0],
    [0, 39671, 8898, 15360, 41601, 6640, 0, 0],
];

pub const COEF_USM_FP16: &'static [[u16; K_FILTER_SIZE]; K_PHASE_COUNT] = &[
    [0, 47309, 15565, 47309, 0, 0, 0, 0],
    [6640, 47326, 15563, 47289, 39408, 0, 0, 0],
    [7429, 47339, 15560, 47266, 40695, 4121, 0, 0],
    [8058, 47349, 15554, 47239, 41286, 0, 0, 0],
    [8387, 47357, 15545, 47209, 41915, 0, 0, 0],
    [8636, 47363, 15534, 47176, 42238, 4121, 0, 0],
    [8767, 47364, 15522, 47141, 42657, 4121, 0, 0],
    [9029, 47367, 15509, 47105, 43023, 4121, 0, 0],
    [9213, 47363, 15490, 47018, 43249, 4121, 0, 0],
    [9280, 47357, 15472, 46928, 43472, 5145, 0, 0],
    [9345, 47347, 15450, 46836, 43727, 5145, 0, 0],
    [9378, 47337, 15427, 46736, 43999, 5669, 0, 0],
    [9437, 47323, 15401, 46630, 44152, 5669, 0, 0],
    [9470, 47310, 15376, 46520, 44312, 6169, 0, 0],
    [9503, 47294, 15338, 46402, 44479, 6378, 0, 0],
    [9503, 47272, 15274, 46280, 44648, 6640, 0, 0],
    [9503, 47253, 15215, 46158, 44817, 6903, 0, 0],
    [9503, 47231, 15150, 45972, 45017, 7165, 0, 0],
    [9535, 47206, 15082, 45708, 45132, 7297, 0, 0],
    [9503, 47180, 15012, 45432, 45232, 7429, 0, 0],
    [9470, 47153, 14939, 45152, 45332, 7560, 0, 0],
    [9470, 47126, 14868, 44681, 45444, 7691, 0, 0],
    [9437, 47090, 14793, 44071, 45560, 7796, 0, 0],
    [9411, 47030, 14714, 42847, 45668, 7927, 0, 0],
    [9411, 46968, 14635, 8387, 45788, 8058, 0, 0],
    [9345, 46902, 14552, 10786, 45908, 8256, 0, 0],
    [9313, 46846, 14478, 11647, 46036, 8321, 0, 0],
    [9247, 46776, 14394, 12292, 46120, 8453, 0, 0],
    [9247, 46714, 14288, 12620, 46184, 8518, 0, 0],
    [9147, 46648, 14130, 12936, 46248, 8570, 0, 0],
    [9029, 46576, 13956, 13268, 46312, 8702, 0, 0],
    [8964, 46512, 13792, 13456, 46378, 8767, 0, 0],
    [8898, 46446, 13624, 13624, 46446, 8898, 0, 0],
    [8767, 46378, 13456, 13792, 46512, 8964, 0, 0],
    [8702, 46312, 13268, 13956, 46576, 9029, 0, 0],
    [8570, 46248, 12936, 14130, 46648, 9147, 0, 0],
    [8518, 46184, 12620, 14288, 46714, 9247, 0, 0],
    [8453, 46120, 12292, 14394, 46776, 9247, 0, 0],
    [8321, 46036, 11647, 14478, 46846, 9313, 0, 0],
    [8256, 45908, 10786, 14552, 46902, 9345, 0, 0],
    [8058, 45788, 8387, 14635, 46968, 9411, 0, 0],
    [7927, 45668, 42847, 14714, 47030, 9411, 0, 0],
    [7796, 45560, 44071, 14793, 47090, 9437, 0, 0],
    [7691, 45444, 44681, 14868, 47126, 9470, 0, 0],
    [7560, 45332, 45152, 14939, 47153, 9470, 0, 0],
    [7429, 45232, 45432, 15012, 47180, 9503, 0, 0],
    [7297, 45132, 45708, 15082, 47206, 9535, 0, 0],
    [7165, 45017, 45972, 15150, 47231, 9503, 0, 0],
    [6903, 44817, 46158, 15215, 47253, 9503, 0, 0],
    [6640, 44648, 46280, 15274, 47272, 9503, 0, 0],
    [6378, 44479, 46402, 15338, 47294, 9503, 0, 0],
    [6169, 44312, 46520, 15376, 47310, 9470, 0, 0],
    [5669, 44152, 46630, 15401, 47323, 9437, 0, 0],
    [5669, 43999, 46736, 15427, 47337, 9378, 0, 0],
    [5145, 43727, 46836, 15450, 47347, 9345, 0, 0],
    [5145, 43472, 46928, 15472, 47357, 9280, 0, 0],
    [4121, 43249, 47018, 15490, 47363, 9213, 0, 0],
    [4121, 43023, 47105, 15509, 47367, 9029, 0, 0],
    [4121, 42657, 47141, 15522, 47364, 8767, 0, 0],
    [4121, 42238, 47176, 15534, 47363, 8636, 0, 0],
    [0, 41915, 47209, 15545, 47357, 8387, 0, 0],
    [0, 41286, 47239, 15554, 47349, 8058, 0, 0],
    [4121, 40695, 47266, 15560, 47339, 7429, 0, 0],
    [0, 39408, 47289, 15563, 47326, 6640, 0, 0],
];

pub fn create_coef_scale_tex(
    texture_cache: &mut ResMut<TextureCache>,
    render_device: &Res<RenderDevice>,
    render_queue: &Res<RenderQueue>,
) -> TextureView {
    create_coef_tex(
        (K_FILTER_SIZE / 4) as u32,
        K_PHASE_COUNT as u32,
        COEF_SCALE,
        texture_cache,
        render_device,
        render_queue,
    )
}

pub fn create_coef_usm_tex(
    texture_cache: &mut ResMut<TextureCache>,
    render_device: &Res<RenderDevice>,
    render_queue: &Res<RenderQueue>,
) -> TextureView {
    create_coef_tex(
        (K_FILTER_SIZE / 4) as u32,
        K_PHASE_COUNT as u32,
        COEF_USM,
        texture_cache,
        render_device,
        render_queue,
    )
}

fn create_coef_tex(
    w: u32,
    h: u32,
    data: &[[f32; K_FILTER_SIZE]; K_PHASE_COUNT],
    texture_cache: &mut ResMut<TextureCache>,
    render_device: &Res<RenderDevice>,
    render_queue: &Res<RenderQueue>,
) -> TextureView {
    let texture_usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;
    let extent = Extent3d {
        width: w,
        height: h,
        depth_or_array_layers: 1,
    };
    let cached_texture = texture_cache.get(
        &render_device,
        TextureDescriptor {
            label: None,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: texture_usage,
        },
    );

    render_queue.write_texture(
        cached_texture.texture.as_image_copy(),
        cast_ref::<[[f32; K_FILTER_SIZE]; K_PHASE_COUNT], [u8; K_IMAGE_SIZE]>(&data),
        ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(NonZeroU32::new(K_ROW_PITCH as u32).unwrap()),
            rows_per_image: None,
        },
        extent,
    );

    cached_texture.default_view
}
