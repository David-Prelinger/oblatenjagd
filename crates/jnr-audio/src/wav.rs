//! RIFF/WAVE (PCM, 16 bit, mono) from and to [`Pcm`].

use crate::render::Pcm;

/// The 44-byte header plus the little-endian samples.
pub fn write(pcm: &Pcm) -> Vec<u8> {
    let data_len = pcm.samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&pcm.rate.to_le_bytes());
    out.extend_from_slice(&(pcm.rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in &pcm.samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Reads a PCM 16-bit mono WAV (other chunks are skipped).
pub fn read(bytes: &[u8]) -> Result<Pcm, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".to_owned());
    }
    let u16_at = |b: &[u8], i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |b: &[u8], i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let mut pos = 12;
    let mut rate = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32_at(bytes, pos + 4) as usize;
        let body = pos + 8;
        let end = body
            .checked_add(len)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| format!("chunk `{}` runs past the end", String::from_utf8_lossy(id)))?;
        match id {
            b"fmt " => {
                if len < 16 {
                    return Err("`fmt ` chunk too short".to_owned());
                }
                let (format, channels, bits) = (
                    u16_at(bytes, body),
                    u16_at(bytes, body + 2),
                    u16_at(bytes, body + 14),
                );
                if (format, channels, bits) != (1, 1, 16) {
                    return Err(format!(
                        "unsupported format {format}, {channels} channels, {bits} bit (need PCM, mono, 16 bit)"
                    ));
                }
                rate = Some(u32_at(bytes, body + 4));
            }
            b"data" => {
                let rate = rate.ok_or("`data` before `fmt `")?;
                if !len.is_multiple_of(2) {
                    return Err("odd `data` length".to_owned());
                }
                let samples = bytes[body..end]
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
                return Ok(Pcm { rate, samples });
            }
            _ => {}
        }
        pos = end + (len & 1); // chunks are padded to even sizes
    }
    Err("no `data` chunk".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let pcm = Pcm {
            rate: 44_100,
            samples: vec![0, 1, -1, i16::MAX, -i16::MAX, i16::MIN, 1234],
        };
        let bytes = write(&pcm);
        assert_eq!(bytes.len(), 44 + 14);
        assert_eq!(read(&bytes), Ok(pcm));
    }

    #[test]
    fn skips_unknown_chunks_and_rejects_bad_files() {
        let pcm = Pcm {
            rate: 22_050,
            samples: vec![5, -5],
        };
        let plain = write(&pcm);
        // Insert an odd-sized `LIST` chunk (padded) between fmt and data.
        let mut with_list = plain[..36].to_vec();
        with_list.extend_from_slice(b"LIST");
        with_list.extend_from_slice(&3u32.to_le_bytes());
        with_list.extend_from_slice(&[1, 2, 3, 0]);
        with_list.extend_from_slice(&plain[36..]);
        assert_eq!(read(&with_list), Ok(pcm));

        assert!(read(b"RIFF").is_err());
        let mut stereo = plain.clone();
        stereo[22] = 2;
        assert!(read(&stereo).unwrap_err().contains("2 channels"));
        assert!(read(&plain[..40]).is_err());
    }
}
