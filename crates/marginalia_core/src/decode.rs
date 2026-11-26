//! Decodificacao de uma pagina de imagem (JPG, PNG, TIFF multipagina, PDF).
//!
//! Paginas sao 1-based. O formato vem da extensao de `path` (sem distinguir
//! maiusculas). PDFs sao renderizados com PDFium (ver `docs/pdfium.md`).

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use image::RgbaImage;
use pdfium_render::prelude::*;
use texts::t;
use thiserror::Error;
use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;
use tiff::ColorType;

/// Maior lado, em pixels, de uma pagina de PDF renderizada.
const PDF_SIDE_MAX: f32 = 2400.0;
/// Teto de pixels (largura x altura) de uma pagina TIFF decodificada: 200 megapixels.
/// Acima disso devolve `NotSupported`, antes de qualquer alocacao.
const MAX_PIXELS: u64 = 200_000_000;
/// Escala natural maxima (pixels por ponto) de uma pagina de PDF.
const PDF_SCALE_MAX: f32 = 2.0;

#[derive(Debug)]
pub struct DecodedPage {
    pub image: RgbaImage,
    pub page_count: u32,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    Invalid(String),
    MissingPage { requested: u32, total: u32 },
    NotSupported(String),
    PdfiumUnavailable(String),
    /// A fonte nao pode ser lida (erro de E/S: NAS desmontado, zip movido). Nao
    /// e defeito da imagem: tentar de novo ou apontar a nova localizacao resolve.
    Reading(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            DecodeError::Invalid(m) => t!("error.decode.invalid", m),
            DecodeError::MissingPage { requested, total } => t!("error.decode.missing_page", requested, total),
            DecodeError::NotSupported(m) => t!("error.decode.not_supported", m),
            DecodeError::PdfiumUnavailable(m) => t!("error.decode.pdfium_unavailable", m),
            DecodeError::Reading(m) => t!("error.decode.reading", m),
        };
        f.write_str(&text)
    }
}

pub fn decode_page(path: &str, bytes: &[u8], page: u32) -> Result<DecodedPage, DecodeError> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let format = match ext.as_str() {
        "jpg" | "jpeg" => Format::Jpg,
        "png" => Format::Png,
        "tif" | "tiff" => Format::Tiff,
        "pdf" => Format::Pdf,
        _ => {
            return Err(DecodeError::NotSupported(t!("error.decode.extension", ext = format!("{ext:?}"), path = format!("{path:?}"))))
        }
    };
    if bytes.is_empty() {
        return Err(DecodeError::Invalid(t!("error.decode.empty_file")));
    }
    // Decodificadores de terceiros nao devem derrubar o app com entrada ruim.
    let r = std::panic::catch_unwind(|| match format {
        Format::Jpg => decode_raster(bytes, image::ImageFormat::Jpeg, page),
        Format::Png => decode_raster(bytes, image::ImageFormat::Png, page),
        Format::Tiff => decode_tiff(bytes, page),
        Format::Pdf => decode_pdf(bytes, page),
    });
    r.unwrap_or_else(|_| Err(DecodeError::Invalid(t!("error.decode.failure_interna"))))
}

#[derive(Clone, Copy)]
enum Format {
    Jpg,
    Png,
    Tiff,
    Pdf,
}

fn page_checks(page: u32, total: u32) -> Result<(), DecodeError> {
    if page == 0 || page > total {
        Err(DecodeError::MissingPage { requested: page, total })
    } else {
        Ok(())
    }
}

fn decode_raster(
    bytes: &[u8],
    fmt: image::ImageFormat,
    page: u32,
) -> Result<DecodedPage, DecodeError> {
    page_checks(page, 1)?;
    let img = image::load_from_memory_with_format(bytes, fmt)
        .map_err(|e| DecodeError::Invalid(e.to_string()))?;
    Ok(DecodedPage { image: img.to_rgba8(), page_count: 1 })
}

fn inv<E: std::fmt::Display>(e: E) -> DecodeError {
    DecodeError::Invalid(e.to_string())
}

/// Erros do crate `tiff`: recursos que ele nao decodifica (compressao como
/// CCITT, paleta, etc.) viram `NotSupported`; o resto e arquivo `Invalid`.
fn tiff_err(e: tiff::TiffError) -> DecodeError {
    match e {
        tiff::TiffError::UnsupportedError(u) => DecodeError::NotSupported(t!("error.decode.tiff_recurso", u)),
        tiff::TiffError::LimitsExceeded => DecodeError::NotSupported(t!("error.decode.tiff_limits")),
        other => DecodeError::Invalid(other.to_string()),
    }
}

fn incomplete() -> DecodeError {
    DecodeError::Invalid(t!("error.decode.pixels_incompletos"))
}

/// Suporta cinza, cinza+alfa, RGB e RGBA de 8 e 16 bits (16 vira 8 com `>> 8`)
/// e cinza de 1 bit. Paleta, CCITT e outras profundidades: `NotSupported`.
fn decode_tiff(bytes: &[u8], page: u32) -> Result<DecodedPage, DecodeError> {
    let mut dec = Decoder::new(Cursor::new(bytes)).map_err(tiff_err)?;
    let mut total = 1u32;
    while dec.more_images() {
        dec.next_image().map_err(tiff_err)?;
        total += 1;
    }
    page_checks(page, total)?;
    let mut dec = Decoder::new(Cursor::new(bytes)).map_err(tiff_err)?;
    for _ in 1..page {
        dec.next_image().map_err(tiff_err)?;
    }
    let (w, h) = dec.dimensions().map_err(tiff_err)?;
    let color = dec.colortype().map_err(tiff_err)?;
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(DecodeError::NotSupported(t!("error.decode.large_image", w, h, MAX_PIXELS)));
    }
    if color == ColorType::Gray(1) {
        // O `tiff` 0.9 trata cada amostra como um byte e nao entende linhas
        // empacotadas de 1 bit; as strips sao lidas e descomprimidas aqui.
        return decode_bilevel(&mut dec, bytes, w, h, total);
    }
    let data = dec.read_image().map_err(tiff_err)?;
    let n = (w as usize).checked_mul(h as usize).ok_or_else(incomplete)?;
    let not_supported = || {
        DecodeError::NotSupported(t!("error.decode.tiff_color", format!("{color:?}")))
    };
    let rgba: Vec<u8> = match (color, data) {
        (ColorType::Gray(8), DecodingResult::U8(d)) if d.len() >= n => {
            d[..n].iter().flat_map(|&g| [g, g, g, 255]).collect()
        }
        (ColorType::GrayA(8), DecodingResult::U8(d)) if d.len() >= n * 2 => {
            d[..n * 2].chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect()
        }
        (ColorType::RGB(8), DecodingResult::U8(d)) if d.len() >= n * 3 => {
            d[..n * 3].chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect()
        }
        (ColorType::RGBA(8), DecodingResult::U8(d)) if d.len() >= n * 4 => d[..n * 4].to_vec(),
        (ColorType::Gray(16), DecodingResult::U16(d)) if d.len() >= n => d[..n]
            .iter()
            .flat_map(|&g| {
                let g = (g >> 8) as u8;
                [g, g, g, 255]
            })
            .collect(),
        (ColorType::GrayA(16), DecodingResult::U16(d)) if d.len() >= n * 2 => d[..n * 2]
            .chunks_exact(2)
            .flat_map(|p| {
                let g = (p[0] >> 8) as u8;
                [g, g, g, (p[1] >> 8) as u8]
            })
            .collect(),
        (ColorType::RGB(16), DecodingResult::U16(d)) if d.len() >= n * 3 => d[..n * 3]
            .chunks_exact(3)
            .flat_map(|p| [(p[0] >> 8) as u8, (p[1] >> 8) as u8, (p[2] >> 8) as u8, 255])
            .collect(),
        (ColorType::RGBA(16), DecodingResult::U16(d)) if d.len() >= n * 4 => {
            d[..n * 4].iter().map(|&v| (v >> 8) as u8).collect()
        }
        (
            ColorType::Gray(8 | 16) | ColorType::GrayA(8 | 16) | ColorType::RGB(8 | 16) | ColorType::RGBA(8 | 16),
            _,
        ) => return Err(incomplete()),
        _ => return Err(not_supported()),
    };
    let image = RgbaImage::from_raw(w, h, rgba).ok_or_else(|| DecodeError::Invalid(t!("error.decode.dimensoes")))?;
    Ok(DecodedPage { image, page_count: total })
}

/// Descomprime PackBits; mais que `expected` bytes de saida e arquivo invalido.
fn unpacks_packbits(src: &[u8], expected: usize) -> Result<Vec<u8>, DecodeError> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i < src.len() && out.len() < expected {
        let n = src[i] as i8;
        i += 1;
        if n >= 0 {
            let end = (i + n as usize + 1).min(src.len());
            out.extend_from_slice(&src[i..end]);
            i = end;
        } else if n != -128 {
            let copies = (1 - i16::from(n)) as usize; // n em -127..=-1 -> 2..=128
            if let Some(&b) = src.get(i) {
                out.extend(std::iter::repeat(b).take(copies));
            }
            i += 1;
        }
        if out.len() > expected {
            return Err(DecodeError::Invalid(t!("error.decode.packbits")));
        }
    }
    Ok(out)
}

/// TIFF de 1 bit (linhas empacotadas MSB primeiro, cada linha alinhada a byte;
/// 1 = branco apos normalizar WhiteIsZero). Compressoes: nenhuma, LZW, PackBits, Deflate.
fn decode_bilevel(
    dec: &mut Decoder<Cursor<&[u8]>>,
    bytes: &[u8],
    w: u32,
    h: u32,
    total: u32,
) -> Result<DecodedPage, DecodeError> {
    use std::io::Read;
    let compression = dec.find_tag_unsigned::<u16>(Tag::Compression).map_err(tiff_err)?.unwrap_or(1);
    let photo = dec.find_tag_unsigned::<u16>(Tag::PhotometricInterpretation).map_err(tiff_err)?.unwrap_or(1);
    if dec.find_tag(Tag::TileWidth).map_err(tiff_err)?.is_some() {
        return Err(DecodeError::NotSupported(t!("error.decode.tiff_tiles")));
    }
    if dec.find_tag_unsigned::<u16>(Tag::FillOrder).map_err(tiff_err)?.is_some_and(|f| f == 2) {
        return Err(DecodeError::NotSupported(t!("error.decode.tiff_fillorder")));
    }
    if dec.find_tag_unsigned::<u16>(Tag::Predictor).map_err(tiff_err)?.is_some_and(|p| p != 1) {
        return Err(DecodeError::NotSupported(t!("error.decode.tiff_predictor")));
    }
    if !matches!(compression, 1 | 5 | 8 | 32773 | 32946) {
        return Err(DecodeError::NotSupported(t!("error.decode.tiff_compression", compression)));
    }
    let offsets = dec.find_tag_unsigned_vec::<u64>(Tag::StripOffsets).map_err(tiff_err)?.unwrap_or_default();
    let counts = dec.find_tag_unsigned_vec::<u64>(Tag::StripByteCounts).map_err(tiff_err)?.unwrap_or_default();
    let by_strip = dec.find_tag_unsigned::<u32>(Tag::RowsPerStrip).map_err(tiff_err)?.unwrap_or(h).max(1);
    let (wz, hz) = (w as usize, h as usize);
    let line = wz.div_ceil(8);
    let size = line.checked_mul(hz).ok_or_else(incomplete)?;
    let n_strips = hz.div_ceil(by_strip as usize);
    if offsets.len() < n_strips || counts.len() < n_strips {
        return Err(incomplete());
    }
    let mut packed: Vec<u8> = Vec::with_capacity(size);
    for i in 0..n_strips {
        let lines = (by_strip as usize).min(hz - i * by_strip as usize);
        let expected = line * lines;
        let ini = usize::try_from(offsets[i]).map_err(|_| incomplete())?;
        let end = ini.checked_add(usize::try_from(counts[i]).map_err(|_| incomplete())?).ok_or_else(incomplete)?;
        let raw = bytes.get(ini..end).ok_or_else(incomplete)?;
        let strip = match compression {
            1 => raw.to_vec(),
            32773 => unpacks_packbits(raw, expected)?,
            5 => {
                // saida limitada a esperado + 1 bytes: o byte extra denuncia expansao excessiva
                let mut buf = vec![0u8; expected + 1];
                let mut dec = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
                let (mut ent, mut leaves) = (raw, 0usize);
                loop {
                    let r = dec.decode_bytes(ent, &mut buf[leaves..]);
                    ent = &ent[r.consumed_in..];
                    leaves += r.consumed_out;
                    match r.status.map_err(inv)? {
                        weezl::LzwStatus::Done => break,
                        _ if leaves > expected => break,
                        _ if r.consumed_in == 0 && r.consumed_out == 0 => break,
                        _ => {}
                    }
                }
                buf.truncate(leaves);
                buf
            }
            _ => {
                let mut v = Vec::with_capacity(expected + 1);
                flate2::read::ZlibDecoder::new(raw)
                    .take(expected as u64 + 1)
                    .read_to_end(&mut v)
                    .map_err(inv)?;
                v
            }
        };
        if strip.len() > expected {
            return Err(DecodeError::Invalid(t!("error.decode.strip")));
        }
        if strip.len() < expected {
            return Err(incomplete());
        }
        packed.extend_from_slice(&strip);
    }
    let inverted = photo == 0;
    let mut rgba: Vec<u8> = Vec::new();
    rgba.try_reserve_exact(wz * hz * 4)
        .map_err(|_| DecodeError::NotSupported(t!("error.decode.no_memory")))?;
    for y in 0..hz {
        for x in 0..wz {
            let bit = (packed[y * line + x / 8] >> (7 - x % 8)) & 1;
            let g = if (bit == 1) != inverted { 255 } else { 0 };
            rgba.extend_from_slice(&[g, g, g, 255]);
        }
    }
    let image = RgbaImage::from_raw(w, h, rgba).ok_or_else(|| DecodeError::Invalid(t!("error.decode.dimensoes")))?;
    Ok(DecodedPage { image, page_count: total })
}

fn as_install() -> String {
    t!("error.decode.pdfium_as_install")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Pastas onde a biblioteca da PDFium e procurada, na ordem: a variavel de
/// ambiente; ao lado do executavel (app instalado, onde o pacote a leva junto) e
/// em `../Frameworks` (dentro de um `.app` do macOS); `vendor/pdfium/lib` do
/// workspace (desenvolvimento). Depois dessas, vem a biblioteca do sistema.
fn pdfium_folders(env: Option<std::ffi::OsString>, exe: Option<&Path>, root_dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = env.filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(d));
    }
    if let Some(folder) = exe.and_then(Path::parent) {
        dirs.push(folder.to_path_buf());
        dirs.push(folder.join("..").join("Frameworks"));
    }
    dirs.push(root_dir.join("vendor").join("pdfium").join("lib"));
    dirs
}

fn load_pdfium() -> Result<Pdfium, DecodeError> {
    let mut attempts: Vec<String> = Vec::new();
    let exe = std::env::current_exe().ok();
    let dirs = pdfium_folders(std::env::var_os("PDFIUM_LIB_DIR"), exe.as_deref(), &workspace_root());
    for d in &dirs {
        match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(d)) {
            Ok(b) => return Ok(Pdfium::new(b)),
            Err(e) => attempts.push(format!("{}: {e}", d.display())),
        }
    }
    match Pdfium::bind_to_system_library() {
        Ok(b) => Ok(Pdfium::new(b)),
        Err(e) => {
            attempts.push(t!("error.decode.pdfium_biblioteca_of_the_sistema", e));
            Err(DecodeError::PdfiumUnavailable(t!("error.decode.pdfium_attempts", as_install(), attempts.join("; "))))
        }
    }
}

/// Envolve o `Pdfium` para poder morar num `static`.
struct PdfiumGlobal(Pdfium);

// SAFETY: o `Pdfium` so e tocado com o `Mutex` abaixo travado (carregar e
// renderizar acontecem inteiros sob o lock), entao nunca ha acesso concorrente
// as bindings nem a PDFium; so o handle muda de thread, e as bindings sao
// ponteiros de funcao de uma biblioteca carregada ate o fim do processo.
unsafe impl Send for PdfiumGlobal {}

/// PDFium nao e thread-safe e as bindings sao criadas uma unica vez: ficam
/// atras de um Mutex global, e todo uso (carregar, renderizar) o segura.
fn pdfium() -> Result<&'static Mutex<PdfiumGlobal>, DecodeError> {
    static CELL: OnceLock<Mutex<PdfiumGlobal>> = OnceLock::new();
    static INIT: Mutex<()> = Mutex::new(());
    if let Some(p) = CELL.get() {
        return Ok(p);
    }
    let _g = INIT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = CELL.get() {
        return Ok(p);
    }
    let p = load_pdfium()?;
    Ok(CELL.get_or_init(|| Mutex::new(PdfiumGlobal(p))))
}

fn decode_pdf(bytes: &[u8], page: u32) -> Result<DecodedPage, DecodeError> {
    let guard = pdfium()?.lock().unwrap_or_else(|e| e.into_inner());
    let doc = guard.0.load_pdf_from_byte_slice(bytes, None).map_err(inv)?;
    let total = u32::from(doc.pages().len());
    page_checks(page, total)?;
    let pg = doc.pages().get((page - 1) as u16).map_err(inv)?;
    let (w_pt, h_pt) = (pg.width().value, pg.height().value);
    if !(w_pt > 0.0 && h_pt > 0.0 && w_pt.is_finite() && h_pt.is_finite()) {
        return Err(DecodeError::Invalid(t!("error.decode.pdf_size")));
    }
    let scale = PDF_SCALE_MAX.min(PDF_SIDE_MAX / w_pt.max(h_pt));
    let w = ((w_pt * scale).round() as i32).clamp(1, PDF_SIDE_MAX as i32);
    let h = ((h_pt * scale).round() as i32).clamp(1, PDF_SIDE_MAX as i32);
    let cfg = PdfRenderConfig::new()
        .set_target_size(w, h)
        .set_format(PdfBitmapFormat::BGRA)
        .set_clear_color(PdfColor::WHITE);
    let bmp = pg.render_with_config(&cfg).map_err(inv)?;
    let (bw, bh) = (bmp.width() as u32, bmp.height() as u32);
    let raw = bmp.as_raw_bytes();
    let stride = raw.len() / bh.max(1) as usize;
    if stride < bw as usize * 4 {
        return Err(DecodeError::Invalid(t!("error.decode.pdf_bitmap")));
    }
    let mut rgba = Vec::with_capacity(bw as usize * bh as usize * 4);
    for line in raw.chunks_exact(stride).take(bh as usize) {
        for px in line[..bw as usize * 4].chunks_exact(4) {
            rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }
    let image = RgbaImage::from_raw(bw, bh, rgba).ok_or_else(|| DecodeError::Invalid(t!("error.decode.pdf_bitmap")))?;
    Ok(DecodedPage { image, page_count: total })
}

#[cfg(test)]
#[path = "tests/decode.rs"]
mod tests;
