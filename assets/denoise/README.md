# Learned denoiser weights

Network weights for the path tracer's learned denoiser (`denoise: "learned"`, brief phase 22). They are run by our own
WGSL compute shaders; no Open Image Denoise code is linked or copied.

| File | Bytes | SHA-256 |
|---|---|---|
| `rt_hdr_alb_nrm.tza` | 1841160 | `e586ef2ff48d7fbb7611986405220ed8fc5c13ca79bfc40be4dc742fbf959e1a` |
| `rt_hdr_alb_nrm_small.tza` | 641480 | `980ea307e7825cae23ed8b1addf8de62ece84b61094795a5631ebf1c91b2e1a7` |

- **Source:** https://github.com/RenderKit/oidn-weights at commit `28883d1769d5930e13cf7f1676dd852bd81ed9e7`
  (2024-04-28), the weights used by Open Image Denoise v2.5.1. Fetched 2026-11-23; each file's SHA-256 equals the
  Git LFS object id in that commit.
- **Licence:** Apache-2.0, Copyright Intel Corporation (`LICENSE-oidn-weights.txt` beside this file).
- **Inputs these two were trained for:** HDR colour + albedo + normal (9 channels in, 3 out).

## File format (`.tza`, read from the files and checked against them)

Little endian throughout.

- Header: `u16` magic `0x41D7`, `u8` major version `2`, `u8` minor version `0`, `u64` offset of the tensor table.
- Tensor data: from byte 64 up to the table, at the offsets the table gives.
- Table (runs to the end of the file): `u32` tensor count, then per tensor: `u16` name length, the name (ASCII),
  `u8` number of dimensions, that many `u32` dimensions, that many layout characters (`oihw` for a convolution
  weight, `x` for a bias), one type character (`h` = IEEE half float; `f` = 32-bit float does not occur here), and
  a `u64` byte offset of the data from the start of the file.

Every convolution is 3 × 3 with zero padding 1 and stride 1.

## Network (from Open Image Denoise v2.5.1, `core/unet_filter.cpp`, `core/color.h`, `devices/cpu/*`)

Both files are the same U-Net with different channel counts; read the counts from the tensor dims.

```
x     = input (9 channels)
x     = relu(enc_conv0(x))
pool1 = maxpool2(relu(enc_conv1(x)))
pool2 = maxpool2(relu(enc_conv2(pool1)))
pool3 = maxpool2(relu(enc_conv3(pool2)))
pool4 = maxpool2(relu(enc_conv4(pool3)))
x     = relu(enc_conv5b(relu(enc_conv5a(pool4))))
x     = relu(dec_conv4b(relu(dec_conv4a(concat(upsample2(x), pool3)))))
x     = relu(dec_conv3b(relu(dec_conv3a(concat(upsample2(x), pool2)))))
x     = relu(dec_conv2b(relu(dec_conv2a(concat(upsample2(x), pool1)))))
x     = relu(dec_conv1b(relu(dec_conv1a(concat(upsample2(x), input)))))
out   = relu(dec_conv0(x))                       (3 channels)
```

`maxpool2` is a 2 × 2 max pool with stride 2; `upsample2` is nearest-neighbour ×2; `concat` puts the upsampled
tensor's channels first. Width and height are padded with zeros to a multiple of 16 before the network and cropped
after it.

**Input channels, in order:** colour r g b, albedo r g b, normal x y z.

- *Colour:* `c = clamp(colour · scale, 0, ∞)` with NaN → 0, then `PU(c) · normScale` per channel.
- *Albedo:* clamped to 0…1, no transfer function.
- *Normal:* clamped to −1…1, then `· 0.5 + 0.5`.
- *Output:* `colour = PUinv(clamp(out, 0, ∞) / normScale) / scale`.

**Autoexposure `scale`:** split the image into bins of at most 16 × 16 pixels (`ceil(H/16)` × `ceil(W/16)` bins, bin
`i` covering rows `i·H/bins … (i+1)·H/bins`), take each bin's mean luminance `L` (0.212671 r + 0.715160 g +
0.072169 b of the sanitised colour), average `log2(L)` over the bins with `L > 1e-8`, and
`scale = 0.18 / 2^average` (1.0 when no bin qualifies).

**Transfer function PU** (constants as in `core/color.h`):

```
a = 1.41283765e+03   b = 1.64593172e+00   c = 4.31384981e-01   d = -2.94139609e-03
e = 1.92653254e-01   f = 6.26026094e-03   g = 9.98620152e-01
y0 = 1.57945760e-06  y1 = 3.22087631e-02  x0 = 2.23151711e-03  x1 = 3.70974749e-01

PU(y)    = a·y                 if y <= y0
           b·y^c + d           if y <= y1
           e·ln(y + f) + g     otherwise
PUinv(x) = x / a               if x <= x0
           ((x − d) / b)^(1/c) if x <= x1
           exp((x − g) / e) − f otherwise
normScale = 1 / PU(65504)
```

## Tensors

`rt_hdr_alb_nrm.tza` (32 tensors):

| tensor | dims | layout | type | offset |
|---|---|---|---|---|
| `enc_conv0.weight` | 32 × 9 × 3 × 3 | oihw | h | 64 |
| `enc_conv0.bias` | 32 | x | h | 5248 |
| `enc_conv1.weight` | 32 × 32 × 3 × 3 | oihw | h | 5312 |
| `enc_conv1.bias` | 32 | x | h | 23744 |
| `enc_conv2.weight` | 48 × 32 × 3 × 3 | oihw | h | 23808 |
| `enc_conv2.bias` | 48 | x | h | 51456 |
| `enc_conv3.weight` | 64 × 48 × 3 × 3 | oihw | h | 51584 |
| `enc_conv3.bias` | 64 | x | h | 106880 |
| `enc_conv4.weight` | 80 × 64 × 3 × 3 | oihw | h | 107008 |
| `enc_conv4.bias` | 80 | x | h | 199168 |
| `enc_conv5a.weight` | 96 × 80 × 3 × 3 | oihw | h | 199360 |
| `enc_conv5a.bias` | 96 | x | h | 337600 |
| `enc_conv5b.weight` | 96 × 96 × 3 × 3 | oihw | h | 337792 |
| `enc_conv5b.bias` | 96 | x | h | 503680 |
| `dec_conv4a.weight` | 112 × 160 × 3 × 3 | oihw | h | 503872 |
| `dec_conv4a.bias` | 112 | x | h | 826432 |
| `dec_conv4b.weight` | 112 × 112 × 3 × 3 | oihw | h | 826688 |
| `dec_conv4b.bias` | 112 | x | h | 1052480 |
| `dec_conv3a.weight` | 96 × 160 × 3 × 3 | oihw | h | 1052736 |
| `dec_conv3a.bias` | 96 | x | h | 1329216 |
| `dec_conv3b.weight` | 96 × 96 × 3 × 3 | oihw | h | 1329408 |
| `dec_conv3b.bias` | 96 | x | h | 1495296 |
| `dec_conv2a.weight` | 64 × 128 × 3 × 3 | oihw | h | 1495488 |
| `dec_conv2a.bias` | 64 | x | h | 1642944 |
| `dec_conv2b.weight` | 64 × 64 × 3 × 3 | oihw | h | 1643072 |
| `dec_conv2b.bias` | 64 | x | h | 1716800 |
| `dec_conv1a.weight` | 64 × 73 × 3 × 3 | oihw | h | 1716928 |
| `dec_conv1a.bias` | 64 | x | h | 1801024 |
| `dec_conv1b.weight` | 32 × 64 × 3 × 3 | oihw | h | 1801152 |
| `dec_conv1b.bias` | 32 | x | h | 1838016 |
| `dec_conv0.weight` | 3 × 32 × 3 × 3 | oihw | h | 1838080 |
| `dec_conv0.bias` | 3 | x | h | 1839808 |

`rt_hdr_alb_nrm_small.tza` (32 tensors):

| tensor | dims | layout | type | offset |
|---|---|---|---|---|
| `enc_conv0.weight` | 32 × 9 × 3 × 3 | oihw | h | 64 |
| `enc_conv0.bias` | 32 | x | h | 5248 |
| `enc_conv1.weight` | 32 × 32 × 3 × 3 | oihw | h | 5312 |
| `enc_conv1.bias` | 32 | x | h | 23744 |
| `enc_conv2.weight` | 32 × 32 × 3 × 3 | oihw | h | 23808 |
| `enc_conv2.bias` | 32 | x | h | 42240 |
| `enc_conv3.weight` | 32 × 32 × 3 × 3 | oihw | h | 42304 |
| `enc_conv3.bias` | 32 | x | h | 60736 |
| `enc_conv4.weight` | 32 × 32 × 3 × 3 | oihw | h | 60800 |
| `enc_conv4.bias` | 32 | x | h | 79232 |
| `enc_conv5a.weight` | 32 × 32 × 3 × 3 | oihw | h | 79296 |
| `enc_conv5a.bias` | 32 | x | h | 97728 |
| `enc_conv5b.weight` | 32 × 32 × 3 × 3 | oihw | h | 97792 |
| `enc_conv5b.bias` | 32 | x | h | 116224 |
| `dec_conv4a.weight` | 64 × 64 × 3 × 3 | oihw | h | 116288 |
| `dec_conv4a.bias` | 64 | x | h | 190016 |
| `dec_conv4b.weight` | 64 × 64 × 3 × 3 | oihw | h | 190144 |
| `dec_conv4b.bias` | 64 | x | h | 263872 |
| `dec_conv3a.weight` | 64 × 96 × 3 × 3 | oihw | h | 264000 |
| `dec_conv3a.bias` | 64 | x | h | 374592 |
| `dec_conv3b.weight` | 64 × 64 × 3 × 3 | oihw | h | 374720 |
| `dec_conv3b.bias` | 64 | x | h | 448448 |
| `dec_conv2a.weight` | 64 × 96 × 3 × 3 | oihw | h | 448576 |
| `dec_conv2a.bias` | 64 | x | h | 559168 |
| `dec_conv2b.weight` | 32 × 64 × 3 × 3 | oihw | h | 559296 |
| `dec_conv2b.bias` | 32 | x | h | 596160 |
| `dec_conv1a.weight` | 32 × 41 × 3 × 3 | oihw | h | 596224 |
| `dec_conv1a.bias` | 32 | x | h | 619840 |
| `dec_conv1b.weight` | 32 × 32 × 3 × 3 | oihw | h | 619904 |
| `dec_conv1b.bias` | 32 | x | h | 638336 |
| `dec_conv0.weight` | 3 × 32 × 3 × 3 | oihw | h | 638400 |
| `dec_conv0.bias` | 3 | x | h | 640128 |
