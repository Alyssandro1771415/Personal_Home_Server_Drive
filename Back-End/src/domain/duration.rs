use std::io::{Read, Seek, SeekFrom};

/// Duração em milissegundos para os contêineres de vídeo mais comuns.
/// Lê só o cabeçalho (mp4/mov, webm/mkv, avi), sem decodificar a mídia.
pub fn video_duration_ms(nome: &str, reader: &mut (impl Read + Seek)) -> Option<i64> {
    let ext = nome
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mp4" | "m4v" | "mov" | "3gp" | "3g2" => mp4_duration_ms(reader),
        "webm" | "mkv" => matroska_duration_ms(reader),
        "avi" => avi_duration_ms(reader),
        _ => None,
    }
}

fn mp4_duration_ms(reader: &mut (impl Read + Seek)) -> Option<i64> {
    let end = reader.seek(SeekFrom::End(0)).ok()?;
    reader.seek(SeekFrom::Start(0)).ok()?;
    walk_mp4(reader, end)
}

fn walk_mp4(reader: &mut (impl Read + Seek), end: u64) -> Option<i64> {
    loop {
        let start = reader.stream_position().ok()?;
        if start + 8 > end {
            return None;
        }
        let mut size = read_u32(reader)? as u64;
        let mut kind = [0u8; 4];
        reader.read_exact(&mut kind).ok()?;
        let mut header = 8u64;
        if size == 1 {
            let mut large = [0u8; 8];
            reader.read_exact(&mut large).ok()?;
            size = u64::from_be_bytes(large);
            header = 16;
        } else if size == 0 {
            size = end.saturating_sub(start);
        }
        if size < header {
            return None;
        }
        let content_end = start.saturating_add(size);
        if content_end > end {
            return None;
        }
        match &kind {
            b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" => {
                if let Some(ms) = walk_mp4(reader, content_end) {
                    return Some(ms);
                }
            }
            b"mvhd" => return parse_mvhd(reader),
            _ => {}
        }
        reader.seek(SeekFrom::Start(content_end)).ok()?;
    }
}

fn parse_mvhd(reader: &mut (impl Read + Seek)) -> Option<i64> {
    let mut version = [0u8; 1];
    reader.read_exact(&mut version).ok()?;
    reader.seek(SeekFrom::Current(3)).ok()?;
    if version[0] == 1 {
        reader.seek(SeekFrom::Current(16)).ok()?;
        let scale = read_u32(reader)?;
        let mut raw = [0u8; 8];
        reader.read_exact(&mut raw).ok()?;
        scale_ms(u64::from_be_bytes(raw), scale)
    } else {
        reader.seek(SeekFrom::Current(8)).ok()?;
        let scale = read_u32(reader)?;
        let duration = read_u32(reader)? as u64;
        scale_ms(duration, scale)
    }
}

fn scale_ms(duration: u64, timescale: u32) -> Option<i64> {
    if timescale == 0 {
        return None;
    }
    i64::try_from(duration as u128 * 1000 / timescale as u128).ok()
}

fn avi_duration_ms(reader: &mut (impl Read + Seek)) -> Option<i64> {
    reader.seek(SeekFrom::Start(0)).ok()?;
    let mut head = [0u8; 4096];
    let n = reader.read(&mut head).ok()?;
    let head = &head[..n];
    let pos = head.windows(4).position(|w| w == b"avih")?;
    let data = head.get(pos + 8..)?;
    if data.len() < 24 {
        return None;
    }
    let micro = u32::from_le_bytes(data[0..4].try_into().ok()?) as u64;
    let frames = u32::from_le_bytes(data[16..20].try_into().ok()?) as u64;
    if micro == 0 || frames == 0 {
        return None;
    }
    i64::try_from(micro.saturating_mul(frames) / 1000).ok()
}

fn matroska_duration_ms(reader: &mut (impl Read + Seek)) -> Option<i64> {
    reader.seek(SeekFrom::Start(0)).ok()?;
    let len = reader.seek(SeekFrom::End(0)).ok()?;
    reader.seek(SeekFrom::Start(0)).ok()?;
    let mut scale: u64 = 1_000_000;
    let mut duration: Option<f64> = None;
    walk_ebml(reader, 0, len, 0, &mut scale, &mut duration)?;
    let duration = duration?;
    if !duration.is_finite() || duration < 0.0 {
        return None;
    }
    let ms = duration * (scale as f64) / 1_000_000.0;
    if !ms.is_finite() || ms > i64::MAX as f64 {
        return None;
    }
    Some(ms.round() as i64)
}

fn walk_ebml(
    reader: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    depth: u8,
    scale: &mut u64,
    duration: &mut Option<f64>,
) -> Option<()> {
    if depth > 8 {
        return Some(());
    }
    reader.seek(SeekFrom::Start(start)).ok()?;
    while reader.stream_position().ok()? + 2 <= end {
        let id = read_id(reader)?;
        let (size, known) = read_data_size(reader)?;
        let here = reader.stream_position().ok()?;
        let child_end = if known {
            let child_end = here.saturating_add(size);
            if child_end > end {
                return None;
            }
            child_end
        } else {
            end
        };
        const SEGMENT: u64 = 0x1853_8067;
        const INFO: u64 = 0x1549_A966;
        const TIMECODE_SCALE: u64 = 0x2AD7_B1;
        const DURATION: u64 = 0x4489;
        const CLUSTER: u64 = 0x1F43_B675;
        if id == CLUSTER {
            break;
        }
        if id == SEGMENT || id == INFO {
            walk_ebml(reader, here, child_end, depth + 1, scale, duration)?;
        } else if id == TIMECODE_SCALE && known && size <= 8 {
            *scale = read_uint(reader, size as usize)?;
        } else if id == DURATION && known && (size == 4 || size == 8) {
            *duration = Some(read_float(reader, size as usize)?);
        }
        if duration.is_some() && *scale > 0 && id == DURATION {
            return Some(());
        }
        reader.seek(SeekFrom::Start(child_end)).ok()?;
    }
    Some(())
}

fn read_id(reader: &mut impl Read) -> Option<u64> {
    let (value, _) = read_vint(reader, false)?;
    Some(value)
}

fn read_data_size(reader: &mut impl Read) -> Option<(u64, bool)> {
    let (value, len) = read_vint(reader, true)?;
    let unknown_mask = if len >= 8 {
        u64::MAX
    } else {
        (1u64 << (7 * len)) - 1
    };
    Some((value, value != unknown_mask))
}

fn read_vint(reader: &mut impl Read, strip_marker: bool) -> Option<(u64, usize)> {
    let mut first = [0u8; 1];
    reader.read_exact(&mut first).ok()?;
    if first[0] == 0 {
        return None;
    }
    let len = first[0].leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let mut buf = [0u8; 8];
    buf[0] = first[0];
    if len > 1 {
        reader.read_exact(&mut buf[1..len]).ok()?;
    }
    let mut value = 0u64;
    for byte in &buf[..len] {
        value = (value << 8) | u64::from(*byte);
    }
    if strip_marker && len < 8 {
        let bits = 7 * len;
        value &= (1u64 << bits) - 1;
    }
    Some((value, len))
}

fn read_uint(reader: &mut impl Read, size: usize) -> Option<u64> {
    if size == 0 || size > 8 {
        return None;
    }
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf[..size]).ok()?;
    let mut value = 0u64;
    for byte in &buf[..size] {
        value = (value << 8) | u64::from(*byte);
    }
    Some(value)
}

fn read_float(reader: &mut impl Read, size: usize) -> Option<f64> {
    if size == 4 {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf).ok()?;
        Some(f32::from_be_bytes(buf) as f64)
    } else if size == 8 {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf).ok()?;
        Some(f64::from_be_bytes(buf))
    } else {
        None
    }
}

fn read_u32(reader: &mut impl Read) -> Option<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).ok()?;
    Some(u32::from_be_bytes(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn be_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = (8 + payload.len()) as u32;
        let mut out = size.to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn reads_mp4_mvhd_duration() {
        let mut mvhd = vec![0u8]; // version 0
        mvhd.extend_from_slice(&[0, 0, 0]); // flags
        mvhd.extend_from_slice(&[0, 0, 0, 1]); // creation
        mvhd.extend_from_slice(&[0, 0, 0, 2]); // modification
        mvhd.extend_from_slice(&1000u32.to_be_bytes()); // timescale
        mvhd.extend_from_slice(&12_500u32.to_be_bytes()); // duration
        let moov = be_box(b"moov", &be_box(b"mvhd", &mvhd));
        let mut cursor = Cursor::new(moov);
        assert_eq!(video_duration_ms("clip.mp4", &mut cursor), Some(12_500));
    }

    #[test]
    fn reads_matroska_duration_in_milliseconds() {
        let mut info = Vec::new();
        info.extend_from_slice(&[0x2A, 0xD7, 0xB1, 0x83, 0x0F, 0x42, 0x40]); // scale 1_000_000
        info.push(0x44);
        info.push(0x89);
        info.push(0x88);
        info.extend_from_slice(&5_000f64.to_be_bytes());
        let mut segment = vec![0x15, 0x49, 0xA9, 0x66];
        segment.push(0x80 | info.len() as u8);
        segment.extend_from_slice(&info);
        let mut root = vec![0x18, 0x53, 0x80, 0x67];
        root.push(0x80 | segment.len() as u8);
        root.extend_from_slice(&segment);
        let mut cursor = Cursor::new(root);
        assert_eq!(video_duration_ms("film.mkv", &mut cursor), Some(5_000));
    }

    #[test]
    fn reads_avi_header_duration() {
        let mut buf = vec![0u8; 64];
        buf[0..4].copy_from_slice(b"avih");
        buf[4..8].copy_from_slice(&56u32.to_le_bytes()); // chunk size, ignored by the scanner offset
        // parser looks at pos+8 for the struct. Our search finds "avih" at 0, data starts at 8.
        // dwMicroSecPerFrame at data[0], dwTotalFrames at data[16].
        let micro = 40_000u32; // 25 fps
        let frames = 50u32; // 2 seconds
        buf[8..12].copy_from_slice(&micro.to_le_bytes());
        buf[24..28].copy_from_slice(&frames.to_le_bytes());
        let mut cursor = Cursor::new(buf);
        assert_eq!(video_duration_ms("clip.avi", &mut cursor), Some(2_000));
    }
}
