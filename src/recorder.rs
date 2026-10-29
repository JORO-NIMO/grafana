//! Local Client Disk Recorder (Zero Cloud Retention)

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::consensus::RecordingType;
use crate::error::{GafanaError, GafanaResult};

pub struct LocalRecorder {
    pub is_recording: bool,
    pub ballot_id: u32,
    pub rec_type: RecordingType,
    pub file_path: String,
    pub bytes_written: u64,
    pub frames_written: u32,
    pub start_timestamp_ms: u32,
    file: Option<File>,
}

impl LocalRecorder {
    pub fn new() -> Self {
        Self {
            is_recording: false,
            ballot_id: 0,
            rec_type: RecordingType::AudioOnly,
            file_path: String::new(),
            bytes_written: 0,
            frames_written: 0,
            start_timestamp_ms: 0,
            file: None,
        }
    }

    /// Start writing stream directly to local disk.
    pub fn start(
        &mut self,
        ballot_id: u32,
        rec_type: RecordingType,
        output_path: &str,
    ) -> GafanaResult<()> {
        if self.is_recording {
            return Err(GafanaError::RecordingActive);
        }

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(output_path)
            .map_err(|_| GafanaError::Io)?;

        let mut bytes_written = 0u64;

        if rec_type == RecordingType::AudioOnly {
            // Standard 44-byte RIFF WAV Header
            let mut wav_hdr = [0u8; 44];
            wav_hdr[0..4].copy_from_slice(b"RIFF");
            wav_hdr[4..8].copy_from_slice(&36u32.to_le_bytes()); // Placeholder
            wav_hdr[8..12].copy_from_slice(b"WAVE");
            wav_hdr[12..16].copy_from_slice(b"fmt ");
            wav_hdr[16..20].copy_from_slice(&16u32.to_le_bytes()); // PCM header size
            wav_hdr[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM format = 1
            wav_hdr[22..24].copy_from_slice(&2u16.to_le_bytes()); // Stereo channels = 2
            wav_hdr[24..28].copy_from_slice(&48000u32.to_le_bytes()); // Sample rate = 48000 Hz
            wav_hdr[28..32].copy_from_slice(&(48000u32 * 2 * 2).to_le_bytes()); // Byte rate
            wav_hdr[32..34].copy_from_slice(&4u16.to_le_bytes()); // Block align = 4
            wav_hdr[34..36].copy_from_slice(&16u16.to_le_bytes()); // 16 bits per sample
            wav_hdr[36..40].copy_from_slice(b"data");
            wav_hdr[40..44].copy_from_slice(&0u32.to_le_bytes()); // Placeholder

            file.write_all(&wav_hdr).map_err(|_| GafanaError::Io)?;
            file.flush().map_err(|_| GafanaError::Io)?;
            bytes_written += 44;
        } else {
            let banner = b"GAFANA_AV_CONTAINER_V1\n";
            file.write_all(banner).map_err(|_| GafanaError::Io)?;
            file.flush().map_err(|_| GafanaError::Io)?;
            bytes_written += banner.len() as u64;
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u32)
            .unwrap_or(0);

        self.is_recording = true;
        self.ballot_id = ballot_id;
        self.rec_type = rec_type;
        self.file_path = output_path.to_string();
        self.bytes_written = bytes_written;
        self.frames_written = 0;
        self.start_timestamp_ms = now_ms;
        self.file = Some(file);

        Ok(())
    }

    /// Enforces anti-capture security: checks if recording is legally permitted under unanimous consensus.
    pub fn is_recording_permitted(&self, consent_state: crate::consensus::ConsentState) -> bool {
        consent_state == crate::consensus::ConsentState::UnanimousOk
    }

    /// Write PCM audio samples to disk sink.
    pub fn write_audio_samples(&mut self, samples: &[i16]) -> GafanaResult<()> {
        if !self.is_recording {
            return Ok(());
        }
        let file = self.file.as_mut().ok_or(GafanaError::Io)?;

        let mut byte_buf = Vec::with_capacity(samples.len() * 2);
        for &s in samples {
            byte_buf.extend_from_slice(&s.to_le_bytes());
        }

        file.write_all(&byte_buf).map_err(|_| GafanaError::Io)?;
        self.bytes_written += byte_buf.len() as u64;
        self.frames_written += 1;
        Ok(())
    }

    /// Write Video frame to disk sink.
    pub fn write_video_frame(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        timestamp_ms: u32,
    ) -> GafanaResult<()> {
        if !self.is_recording || self.rec_type == RecordingType::AudioOnly {
            return Ok(());
        }
        let file = self.file.as_mut().ok_or(GafanaError::Io)?;

        let mut meta = [0u8; 16];
        meta[0..4].copy_from_slice(&width.to_le_bytes());
        meta[4..8].copy_from_slice(&height.to_le_bytes());
        meta[8..12].copy_from_slice(&(data.len() as u32).to_le_bytes());
        meta[12..16].copy_from_slice(&timestamp_ms.to_le_bytes());

        file.write_all(&meta).map_err(|_| GafanaError::Io)?;
        file.write_all(data).map_err(|_| GafanaError::Io)?;
        self.bytes_written += 16 + data.len() as u64;
        self.frames_written += 1;
        Ok(())
    }

    /// Finalize and close local recording file.
    pub fn stop(&mut self) -> GafanaResult<()> {
        if !self.is_recording {
            return Err(GafanaError::RecordingInactive);
        }

        if let Some(mut file) = self.file.take() {
            if self.rec_type == RecordingType::AudioOnly && self.bytes_written >= 44 {
                let data_bytes = (self.bytes_written - 44) as u32;
                let riff_bytes = data_bytes + 36;

                // Seek to offset 4: update RIFF total size
                let _ = file.seek(SeekFrom::Start(4));
                let _ = file.write_all(&riff_bytes.to_le_bytes());

                // Seek to offset 40: update WAV data chunk size
                let _ = file.seek(SeekFrom::Start(40));
                let _ = file.write_all(&data_bytes.to_le_bytes());
            }

            let _ = file.flush();
        }

        self.is_recording = false;
        Ok(())
    }
}
