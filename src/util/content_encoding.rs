use brotli2::stream::{CompressMode as BrotliCompressMode, CompressParams as BrotliCompressParams};
use flate2::write::{DeflateEncoder, GzEncoder};
use flate2::Compression as Flate2Compression;
use iron::headers::{QualityItem, Encoding};
use bzip2::Compression as BzCompression;
use brotli2::write::BrotliEncoder;
use bzip2::write::BzEncoder;
use std::io::{self, Write};
use std::path::Path;
use std::fs::File;
use md6::Md6;


lazy_static! {
    /// The list of content encodings we handle.
    pub static ref SUPPORTED_ENCODINGS: Vec<Encoding> = {
        let es = vec![Encoding::Gzip, Encoding::Deflate, Encoding::EncodingExt("br".to_string()), Encoding::EncodingExt("bzip2".to_string())];
        [es.clone(), es.into_iter().map(|e| Encoding::EncodingExt(format!("x-{}", e))).collect()].into_iter().flat_map(|e| e.clone()).collect()
    };
}

/// The minimal size at which to encode filesystem files.
pub static MIN_ENCODING_SIZE: u64 = 1024;


/// Find best supported encoding to use, or `None` for identity.
pub fn response_encoding(requested: &mut [QualityItem<Encoding>]) -> Option<Encoding> {
    requested.sort_by_key(|e| e.quality);
    requested.iter().filter(|e| e.quality.0 != 0).find(|e| SUPPORTED_ENCODINGS.contains(&e.item)).map(|e| e.item.clone())
}

/// Encode a string slice using a specified encoding or `None` if encoding failed or is not recognised.
pub fn encode_str(dt: &str, enc: &Encoding) -> Option<Vec<u8>> {
    static STR_ENCODING_FNS: &'static [fn(&str) -> Option<Vec<u8>>] = &[encode_str_gzip, encode_str_deflate, encode_str_brotli, encode_str_bzip2];

    encoding_idx(enc).and_then(|fi| STR_ENCODING_FNS[fi](dt))
}

/// Encode the file denoted by the specified path into the file denoted by the specified path using a specified encoding or
/// `false` if encoding failed, is not recognised or an I/O error occurred.
pub fn encode_file(p: &Path, op: &Path, enc: &Encoding) -> bool {
    static FILE_ENCODING_FNS: &'static [fn(File, File) -> bool] = &[encode_file_gzip, encode_file_deflate, encode_file_brotli, encode_file_bzip2];

    encoding_idx(enc)
        .map(|fi| {
            let inf = File::open(p);
            let outf = File::create(op);

            inf.is_ok() && outf.is_ok() && FILE_ENCODING_FNS[fi](inf.unwrap(), outf.unwrap())
        })
        .unwrap()
}

/// Encoding extension to use for encoded files, for example "gz" for gzip, or `None` if the encoding is not recognised.
pub fn encoding_extension(enc: &Encoding) -> Option<&'static str> {
    static ENCODING_EXTS: &'static [&'static str] = &["gz", "dflt", "br", "bz2"];

    encoding_idx(enc).map(|ei| ENCODING_EXTS[ei])
}

/// Return the 256-bit MD6 hash of the file denoted by the specified path.
pub fn file_hash(p: &Path) -> [u8; 32] {
    let mut ctx = Md6::new(256).unwrap();
    let mut res = [0; 32];

    io::copy(&mut File::open(p).unwrap(), &mut ctx).unwrap();
    ctx.finalise(&mut res);

    res
}

/// Create a hash string out of its raw bytes.
///
/// # Examples
///
/// ```
/// use https::util::hash_string;
/// assert_eq!(hash_string(&[0x99, 0xAA, 0xBB, 0xCC]), "99AABBCC".to_string());
/// assert_eq!(hash_string(&[0x09, 0x0A]), "090A".to_string());
/// ```
pub fn hash_string(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut result = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(result, "{:02X}", b).unwrap();
    }
    result
}


fn encoding_idx(enc: &Encoding) -> Option<usize> {
    match *enc {
        Encoding::Gzip => Some(0),
        Encoding::Deflate => Some(1),
        Encoding::EncodingExt(ref e) => {
            match &e[..] {
                "x-gzip" => Some(0),
                "x-deflate" => Some(1),
                "br" | "x-br" => Some(2),
                "bzip2" | "x-bzip2" => Some(3),
                _ => None,
            }
        }
        _ => None,
    }
}

macro_rules! encode_fn_flate2_write_iface {
    ($str_fn_name:ident, $file_fn_name:ident, $enc_tp:ident) => {
        fn $str_fn_name(dt: &str) -> Option<Vec<u8>> {
            let mut cmp = $enc_tp::new(Vec::new(), Flate2Compression::Default);
            cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
        }

        fn $file_fn_name(mut inf: File, outf: File) -> bool {
            let mut cmp = $enc_tp::new(outf, Flate2Compression::Default);
            io::copy(&mut inf, &mut cmp).and_then(|_| cmp.finish()).is_ok()
        }
    }
}

encode_fn_flate2_write_iface!(encode_str_gzip, encode_file_gzip, GzEncoder);
encode_fn_flate2_write_iface!(encode_str_deflate, encode_file_deflate, DeflateEncoder);

fn encode_str_brotli(dt: &str) -> Option<Vec<u8>> {
    let mut cmp = BrotliEncoder::new(Vec::new(), 0);
    cmp.set_params(BrotliCompressParams::new().mode(BrotliCompressMode::Text));
    cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
}

fn encode_file_brotli(mut inf: File, outf: File) -> bool {
    let mut cmp = BrotliEncoder::new(outf, 0);
    cmp.set_params(BrotliCompressParams::new().mode(BrotliCompressMode::Text));
    io::copy(&mut inf, &mut cmp).and_then(|_| cmp.finish()).is_ok()
}

fn encode_str_bzip2(dt: &str) -> Option<Vec<u8>> {
    let mut cmp = BzEncoder::new(Vec::new(), BzCompression::Default);
    cmp.write_all(dt.as_bytes()).ok().and_then(|_| cmp.finish().ok())
}

fn encode_file_bzip2(mut inf: File, outf: File) -> bool {
    let mut cmp = BzEncoder::new(outf, BzCompression::Default);
    io::copy(&mut inf, &mut cmp).and_then(|_| cmp.finish()).is_ok()
}
