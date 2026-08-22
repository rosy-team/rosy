//! File I/O runtime support for Rosy.
//!
//! Provides a file handle registry that maps COSY-style unit numbers
//! to Rust file handles. Supports both ASCII and binary I/O modes.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::sync::Mutex;
use anyhow::{Result, Context, bail};


/// Global file handle registry, mapping unit numbers to open file handles.
static FILE_REGISTRY: Mutex<Option<HashMap<u64, FileHandle>>> = Mutex::new(None);

/// Represents an open file with its mode information.
struct FileHandle {
    reader: Option<BufReader<File>>,
    writer: Option<BufWriter<File>>,
    _path: String,
    _is_binary: bool,
}

fn ensure_registry() {
    let mut reg = FILE_REGISTRY.lock().unwrap();
    if reg.is_none() {
        *reg = Some(HashMap::new());
    }
}

/// Open a file for ASCII I/O (OPENF).
///
/// Arguments follow COSY/Fortran conventions:
/// - `unit`: unit number (integer)
/// - `filename`: path to the file
/// - `status`: Fortran-style status string:
///   - `'unknown'`: read+write; open existing as-is, or create empty if it doesn't exist
///   - `'old'`: open existing file for reading
///   - `'new'`: create new file for writing, error if it already exists
///   - `'replace'`: create or truncate, then open for writing
pub fn rosy_openf(
    unit: impl crate::IntoF64,
    filename: impl crate::RecstFmt,
    status: impl crate::RecstFmt,
) -> Result<()> {
    open_file_impl(unit.into_f64(), &filename.recst_fmt(), &status.recst_fmt(), false)
}

/// Open a file for binary I/O (OPENFB).
pub fn rosy_openfb(
    unit: impl crate::IntoF64,
    filename: impl crate::RecstFmt,
    status: impl crate::RecstFmt,
) -> Result<()> {
    open_file_impl(unit.into_f64(), &filename.recst_fmt(), &status.recst_fmt(), true)
}

fn open_file_impl(unit: f64, filename: &str, status: &str, is_binary: bool) -> Result<()> {
    ensure_registry();
    let unit_num = unit as u64;
    let status_lower = status.to_lowercase();

    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    // Close any existing file on this unit
    if registry.contains_key(&unit_num) {
        registry.remove(&unit_num);
    }

    match status_lower.as_str() {
        "unknown" => {
            // Fortran-style 'UNKNOWN': open for read+write. Existing file
            // contents are preserved (no truncation); a new file is created
            // empty if needed. The reader and writer share the same OS file
            // offset via `try_clone`.
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(filename)
                .with_context(|| format!("Failed to open file '{}' (unit {})", filename, unit_num))?;

            let read_handle = file.try_clone()
                .with_context(|| format!("Failed to clone file handle for '{}' (unit {})", filename, unit_num))?;

            registry.insert(unit_num, FileHandle {
                reader: Some(BufReader::new(read_handle)),
                writer: Some(BufWriter::new(file)),
                _path: filename.to_string(),
                _is_binary: is_binary,
            });
        }
        "replace" => {
            // Always create or truncate, then open for writing.
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(filename)
                .with_context(|| format!("Failed to open file '{}' for writing (unit {})", filename, unit_num))?;

            registry.insert(unit_num, FileHandle {
                reader: None,
                writer: Some(BufWriter::new(file)),
                _path: filename.to_string(),
                _is_binary: is_binary,
            });
        }
        "old" => {
            // Open existing for reading
            let file = File::open(filename)
                .with_context(|| format!("Failed to open existing file '{}' for reading (unit {})", filename, unit_num))?;

            registry.insert(unit_num, FileHandle {
                reader: Some(BufReader::new(file)),
                writer: None,
                _path: filename.to_string(),
                _is_binary: is_binary,
            });
        }
        "new" => {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(filename)
                .with_context(|| format!("Failed to create new file '{}' (unit {}). File may already exist.", filename, unit_num))?;

            registry.insert(unit_num, FileHandle {
                reader: None,
                writer: Some(BufWriter::new(file)),
                _path: filename.to_string(),
                _is_binary: is_binary,
            });
        }
        _ => bail!("Unknown file status '{}' for OPENF/OPENFB. Expected 'unknown', 'old', 'new', or 'replace'.", status),
    }

    Ok(())
}

/// Rewind a file to the beginning (REWF).
pub fn rosy_rewf(unit: f64) -> Result<()> {
    ensure_registry();
    let unit_num = unit as u64;

    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    if let Some(handle) = registry.get_mut(&unit_num) {
        if let Some(ref mut reader) = handle.reader {
            reader.seek(SeekFrom::Start(0))
                .with_context(|| format!("Failed to rewind file on unit {}", unit_num))?;
        } else if let Some(ref mut writer) = handle.writer {
            writer.flush()
                .with_context(|| format!("Failed to flush before rewind on unit {}", unit_num))?;
            writer.get_mut().seek(SeekFrom::Start(0))
                .with_context(|| format!("Failed to rewind writer on unit {}", unit_num))?;
        }
        Ok(())
    } else {
        // COSY silently ignores rewind on an unopened unit
        Ok(())
    }
}

/// Backspace a file by one record (BACKF).
///
/// Seeks the BufReader back to the start of the previous line.
/// All seeks go through the BufReader (not get_mut()) so its internal
/// buffer is flushed and the logical position stays consistent.
pub fn rosy_backf(unit: impl crate::IntoF64) -> Result<()> {
    let unit = unit.into_f64();
    ensure_registry();
    let unit_num = unit as u64;

    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    if let Some(handle) = registry.get_mut(&unit_num) {
        if let Some(ref mut reader) = handle.reader {
            // stream_position() goes through BufReader and reflects the
            // logical read position (i.e. after any prior read_line calls).
            let current_pos = reader.stream_position()
                .with_context(|| format!("Failed to get stream position on unit {}", unit_num))?;

            if current_pos == 0 {
                return Ok(()); // Already at start
            }

            // Seek to the beginning through BufReader so its buffer is flushed.
            reader.seek(SeekFrom::Start(0))
                .with_context(|| format!("Failed to seek to start for backf scan on unit {}", unit_num))?;

            // Read all bytes up to current_pos through BufReader.
            let mut buf = vec![0u8; current_pos as usize];
            reader.read_exact(&mut buf)
                .with_context(|| format!("Failed to read bytes for backf scan on unit {}", unit_num))?;

            // The buffer ends exactly at current_pos, which is just after the
            // newline that terminated the last record we read.  We must skip
            // that trailing newline when searching backwards, otherwise we'd
            // land at the start of the record we just read rather than the one
            // before it.  Trim one trailing newline (and optional \r) before
            // searching.
            let search_end = buf
                .iter()
                .rposition(|&b| b == b'\n')
                .unwrap_or(buf.len());
            let newline_pos = buf[..search_end]
                .iter()
                .rposition(|&b| b == b'\n')
                .map(|p| p as u64 + 1)
                .unwrap_or(0);

            // Seek to that position through BufReader.
            reader.seek(SeekFrom::Start(newline_pos))
                .with_context(|| format!("Failed to seek to previous record on unit {}", unit_num))?;

            Ok(())
        } else {
            // For write units, BACKF is a no-op in Rosy
            Ok(())
        }
    } else {
        Ok(())
    }
}

/// Close a file (CLOSEF).
pub fn rosy_closef(unit: f64) -> Result<()> {
    ensure_registry();
    let unit_num = unit as u64;

    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    if let Some(mut handle) = registry.remove(&unit_num) {
        // Flush the writer if present
        if let Some(ref mut writer) = handle.writer {
            writer.flush()
                .with_context(|| format!("Failed to flush file on unit {} before closing", unit_num))?;
        }
        Ok(())
    } else {
        // COSY doesn't error on closing an unopened unit
        Ok(())
    }
}

/// Write a string to a file unit (ASCII WRITE to file).
pub fn rosy_write_to_unit(unit: impl crate::AsF64, content: impl crate::RecstFmt) -> Result<()> {
    let unit = crate::rosy_as_u64(&unit);
    let content = content.recst_fmt();
    rosy_write_to_unit_str(unit, &content)
}

fn rosy_write_to_unit_str(unit: u64, content: &str) -> Result<()> {
    ensure_registry();
    
    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    let handle = registry.get_mut(&unit)
        .with_context(|| format!("No file open on unit {}. Use OPENF to open a file first.", unit))?;
    
    let writer = handle.writer.as_mut()
        .with_context(|| format!("File on unit {} is not open for writing (opened as 'old'?)", unit))?;
    
    writeln!(writer, "{}", content)
        .with_context(|| format!("Failed to write to file on unit {}", unit))?;
    writer
        .flush()
        .with_context(|| format!("Failed to flush file on unit {}", unit))?;

    Ok(())
}

/// Read a line from a file unit (ASCII READ from file).
/// Returns the trimmed line as a string. Bails on EOF — used by READ
/// (numerical / DA reads) and DAREA / DAPRV header parsing where missing
/// data is genuinely an error.
pub fn rosy_read_from_unit(unit: u64) -> Result<String> {
    ensure_registry();

    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    let handle = registry.get_mut(&unit)
        .with_context(|| format!("No file open on unit {}. Use OPENF to open a file first.", unit))?;

    let reader = handle.reader.as_mut()
        .with_context(|| format!("File on unit {} is not open for reading (opened as 'unknown'?)", unit))?;

    let mut line = String::new();
    let bytes_read = reader.read_line(&mut line)
        .with_context(|| format!("Failed to read from file on unit {}", unit))?;

    if bytes_read == 0 {
        bail!("End of file reached on unit {}", unit);
    }

    Ok(line.trim_end_matches('\n').trim_end_matches('\r').to_string())
}

/// READS variant: returns empty string on EOF instead of bailing.
///
/// This matches cosy.fox semantics where the canonical line-by-line read
/// pattern is `WHILE LIN # ''; READS 77 LIN; ...; ENDWHILE;` — the loop
/// guard relies on READS yielding an empty string at EOF so the WHILE
/// terminates gracefully. The strict `rosy_read_from_unit` above bails on
/// EOF, breaking that idiom; libcosy's COPYF / FILE2VE / RFILT / FGDATIN
/// all assume the empty-on-EOF semantic.
pub fn rosy_reads_string_from_unit(unit: u64) -> Result<String> {
    ensure_registry();

    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    let handle = registry.get_mut(&unit)
        .with_context(|| format!("No file open on unit {}. Use OPENF to open a file first.", unit))?;

    let reader = handle.reader.as_mut()
        .with_context(|| format!("File on unit {} is not open for reading (opened as 'unknown'?)", unit))?;

    let mut line = String::new();
    let bytes_read = reader.read_line(&mut line)
        .with_context(|| format!("Failed to read from file on unit {}", unit))?;

    if bytes_read == 0 {
        return Ok(String::new());
    }

    Ok(line.trim_end_matches('\n').trim_end_matches('\r').to_string())
}

/// Write binary data to a file unit (WRITEB).
pub fn rosy_writeb_to_unit(unit: u64, data: &[u8]) -> Result<()> {
    ensure_registry();
    
    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    let handle = registry.get_mut(&unit)
        .with_context(|| format!("No file open on unit {}. Use OPENFB to open a file first.", unit))?;
    
    let writer = handle.writer.as_mut()
        .with_context(|| format!("File on unit {} is not open for writing", unit))?;
    
    // Write the length prefix (8 bytes, little-endian u64)
    let len = data.len() as u64;
    writer.write_all(&len.to_le_bytes())
        .with_context(|| format!("Failed to write length prefix to file on unit {}", unit))?;
    
    // Write the data
    writer.write_all(data)
        .with_context(|| format!("Failed to write binary data to file on unit {}", unit))?;
    
    Ok(())
}

/// Read binary data from a file unit (READB).
/// Returns the raw bytes.
pub fn rosy_readb_from_unit(unit: u64) -> Result<Vec<u8>> {
    ensure_registry();
    
    let mut reg = FILE_REGISTRY.lock().unwrap();
    let registry = reg.as_mut().unwrap();

    let handle = registry.get_mut(&unit)
        .with_context(|| format!("No file open on unit {}. Use OPENFB to open a file first.", unit))?;
    
    let reader = handle.reader.as_mut()
        .with_context(|| format!("File on unit {} is not open for reading", unit))?;
    
    // Read length prefix
    let mut len_buf = [0u8; 8];
    reader.read_exact(&mut len_buf)
        .with_context(|| format!("Failed to read length prefix from file on unit {}", unit))?;
    let len = u64::from_le_bytes(len_buf) as usize;
    
    // Read the data
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data)
        .with_context(|| format!("Failed to read binary data from file on unit {}", unit))?;
    
    Ok(data)
}

/// Trait for serializing Rosy types to binary.
pub trait RosyToBinary {
    fn to_binary(&self) -> Vec<u8>;
}

/// Trait for deserializing Rosy types from binary.
pub trait RosyFromBinary: Sized {
    fn from_binary(data: &[u8]) -> Result<Self>;
}

// Binary serialization for RE (f64)
impl RosyToBinary for f64 {
    fn to_binary(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

impl RosyFromBinary for f64 {
    fn from_binary(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            bail!("Not enough data to deserialize f64: expected 8 bytes, got {}", data.len());
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&data[..8]);
        Ok(f64::from_le_bytes(buf))
    }
}

// Binary serialization for String
impl RosyToBinary for String {
    fn to_binary(&self) -> Vec<u8> {
        let mut bytes = (self.len() as u64).to_le_bytes().to_vec();
        bytes.extend(self.as_bytes());
        bytes
    }
}

impl RosyFromBinary for String {
    fn from_binary(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            bail!("Not enough data to deserialize String length prefix");
        }
        let mut len_buf = [0u8; 8];
        len_buf.copy_from_slice(&data[..8]);
        let len = u64::from_le_bytes(len_buf) as usize;
        if data.len() < 8 + len {
            bail!("Not enough data to deserialize String body");
        }
        String::from_utf8(data[8..8+len].to_vec())
            .context("Failed to deserialize String from binary")
    }
}

// Binary serialization for VE (Vec<f64>)
impl RosyToBinary for Vec<f64> {
    fn to_binary(&self) -> Vec<u8> {
        let mut bytes = (self.len() as u64).to_le_bytes().to_vec();
        for val in self {
            bytes.extend(val.to_le_bytes());
        }
        bytes
    }
}

impl RosyFromBinary for Vec<f64> {
    fn from_binary(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            bail!("Not enough data to deserialize Vec<f64> length prefix");
        }
        let mut len_buf = [0u8; 8];
        len_buf.copy_from_slice(&data[..8]);
        let len = u64::from_le_bytes(len_buf) as usize;
        let mut result = Vec::with_capacity(len);
        let mut offset = 8;
        for _ in 0..len {
            if offset + 8 > data.len() {
                bail!("Not enough data to deserialize Vec<f64> element");
            }
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&data[offset..offset+8]);
            result.push(f64::from_le_bytes(buf));
            offset += 8;
        }
        Ok(result)
    }
}

// Binary serialization for bool
impl RosyToBinary for bool {
    fn to_binary(&self) -> Vec<u8> {
        vec![if *self { 1 } else { 0 }]
    }
}

impl RosyFromBinary for bool {
    fn from_binary(data: &[u8]) -> Result<Self> {
        if data.is_empty() {
            bail!("Not enough data to deserialize bool");
        }
        Ok(data[0] != 0)
    }
}

// Binary serialization for CM (Complex64): 8 bytes real + 8 bytes imag (little-endian)
impl RosyToBinary for num_complex::Complex64 {
    fn to_binary(&self) -> Vec<u8> {
        let mut bytes = self.re.to_le_bytes().to_vec();
        bytes.extend(self.im.to_le_bytes());
        bytes
    }
}

impl RosyFromBinary for num_complex::Complex64 {
    fn from_binary(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            bail!("Not enough data to deserialize Complex64: expected 16 bytes, got {}", data.len());
        }
        let mut re_buf = [0u8; 8];
        let mut im_buf = [0u8; 8];
        re_buf.copy_from_slice(&data[..8]);
        im_buf.copy_from_slice(&data[8..16]);
        Ok(num_complex::Complex64::new(f64::from_le_bytes(re_buf), f64::from_le_bytes(im_buf)))
    }
}

// Binary serialization for DA (stub — DA binary I/O not yet implemented)
impl RosyToBinary for crate::taylor::DA {
    fn to_binary(&self) -> Vec<u8> {
        panic!("Binary I/O for DA is not yet supported in Rosy. Use DAPRV/DAREV for ASCII DA I/O.")
    }
}

impl RosyFromBinary for crate::taylor::DA {
    fn from_binary(_data: &[u8]) -> Result<Self> {
        bail!("Binary I/O for DA is not yet supported in Rosy. Use DAPRV/DAREV for ASCII DA I/O.")
    }
}
