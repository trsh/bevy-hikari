// The MIT License(MIT)
//
// Copyright(c) 2022 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of
// this software and associated documentation files(the "Software"), to deal in
// the Software without restriction, including without limitation the rights to
// use, copy, modify, merge, publish, distribute, sublicense, and / or sell copies of
// the Software, and to permit persons to whom the Software is furnished to do so,
// subject to the following conditions :
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
// FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.IN NO EVENT SHALL THE AUTHORS OR
// COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
// IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//---------------------------------------------------------------------------------
// NVIDIA Image Scaling SDK  - v1.0.3
//---------------------------------------------------------------------------------
// HLSL main example
//---------------------------------------------------------------------------------

#define NIS_HLSL 1
//#define NIS_USE_HALF_PRECISION 1
#define NIS_CLAMP_OUTPUT 1

#ifndef NIS_SCALER
#define NIS_SCALER 1
#endif

#ifndef NIS_DXC
#define NIS_DXC 1
#endif

#if NIS_DXC
#define NIS_PUSH_CONSTANT    [[vk::push_constant]]
#define NIS_BINDING(bindingIndex, group) [[vk::binding(bindingIndex, group)]]
#else
#define NIS_PUSH_CONSTANT
#define NIS_BINDING(bindingIndex)
#endif


NIS_BINDING(0, 1) cbuffer cb : register(b0, space1)
{
    float kDetectRatio;
    float kDetectThres;
    float kMinContrastRatio;
    float kRatioNorm;

    float kContrastBoost;
    float kEps;
    float kSharpStartY;
    float kSharpScaleY;

    float kSharpStrengthMin;
    float kSharpStrengthScale;
    float kSharpLimitMin;
    float kSharpLimitScale;

    float kScaleX;
    float kScaleY;

    float kDstNormX;
    float kDstNormY;
    float kSrcNormX;
    float kSrcNormY;

    uint kInputViewportOriginX;
    uint kInputViewportOriginY;
    uint kInputViewportWidth;
    uint kInputViewportHeight;

    uint kOutputViewportOriginX;
    uint kOutputViewportOriginY;
    uint kOutputViewportWidth;
    uint kOutputViewportHeight;

    float reserved0;
    float reserved1;
};

NIS_BINDING(1, 0) SamplerState samplerLinearClamp : register(s1, space0);
#if NIS_NV12_SUPPORT
//NIS_BINDING(2) Texture2D<float> in_texture_y   : register(t0);
//NIS_BINDING(2) Texture2D<float2> in_texture_uv : register(t3);
#else
NIS_BINDING(1, 1) Texture2D in_texture            : register(t1, space1);
#endif
[[spv::format_rgba16f]] NIS_BINDING(0, 2) RWTexture2D<float4> out_texture : register(u0, space2);
//#if NIS_SCALER
NIS_BINDING(2, 1) Texture2D coef_scaler           : register(t2, space1);
NIS_BINDING(3, 1) Texture2D coef_usm              : register(t3, space1);
//#endif



#include "NIS_Scaler.h"

[numthreads(NIS_THREAD_GROUP_SIZE, 1, 1)]
void main(uint3 blockIdx : SV_GroupID, uint3 threadIdx : SV_GroupThreadID)
{
#if NIS_SCALER
    NVScaler(blockIdx.xy, threadIdx.x);
#else
    NVSharpen(blockIdx.xy, threadIdx.x);
#endif
}