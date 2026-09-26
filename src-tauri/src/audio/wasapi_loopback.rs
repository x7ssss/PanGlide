use crate::audio::resampler::AudioResampler;
use crate::error::{PanGlideError, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IAudioClock,
    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_HIGHEST};

#[derive(Clone, Debug)]
pub struct AudioPacket {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp_100ns: i64,
    pub hardware_audio_clock_ticks: u64,
}

pub type AudioCallback = Arc<dyn Fn(AudioPacket) + Send + Sync + 'static>;

pub struct WasapiLoopbackRecorder {
    is_running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    latest_clock_pos: Arc<Mutex<u64>>,
    clock_frequency: Arc<Mutex<u64>>,
}

impl WasapiLoopbackRecorder {
    pub fn start(on_audio: AudioCallback) -> Result<Self> {
        let is_running = Arc::new(AtomicBool::new(true));
        let running_flag = is_running.clone();
        let latest_clock_pos = Arc::new(Mutex::new(0u64));
        let clock_pos_clone = latest_clock_pos.clone();
        let clock_frequency = Arc::new(Mutex::new(48000u64));
        let clock_freq_clone = clock_frequency.clone();

        let thread_handle = thread::Builder::new()
            .name("panglide-wasapi-loopback".into())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                    let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
                }

                let run_result = Self::run_loop(
                    running_flag,
                    clock_pos_clone,
                    clock_freq_clone,
                    on_audio,
                );

                if let Err(e) = run_result {
                    eprintln!("[WasapiLoopbackRecorder] error: {:?}", e);
                }

                unsafe {
                    CoUninitialize();
                }
            })
            .map_err(|e| PanGlideError::Audio(format!("Failed to spawn loopback thread: {}", e)))?;

        Ok(Self {
            is_running,
            thread_handle: Some(thread_handle),
            latest_clock_pos,
            clock_frequency,
        })
    }

    fn run_loop(
        is_running: Arc<AtomicBool>,
        clock_pos: Arc<Mutex<u64>>,
        clock_freq: Arc<Mutex<u64>>,
        on_audio: AudioCallback,
    ) -> Result<()> {
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
            let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

            let mix_format_ptr = audio_client.GetMixFormat()?;
            let mix_format = *mix_format_ptr;
            let sample_rate = mix_format.nSamplesPerSec;
            let channels = mix_format.nChannels;
            let bits_per_sample = mix_format.wBitsPerSample;

            // 100ms buffer in 100ns units
            let buffer_duration: i64 = 1_000_000;
            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                buffer_duration,
                0,
                mix_format_ptr,
                None,
            )?;

            let capture_client: IAudioCaptureClient = audio_client.GetService()?;
            let audio_clock: Result<IAudioClock> = audio_client.GetService().map_err(Into::into);

            if let Ok(ref clock) = audio_clock {
                if let Ok(freq) = clock.GetFrequency() {
                    if freq > 0 {
                        *clock_freq.lock().unwrap() = freq;
                    }
                }
            }

            // Keep a silent render client active on the render endpoint so the WASAPI loopback clock
            // continuously runs and delivers packets even when no other application is playing audio.
            let silent_render_client: Option<IAudioClient> = device.Activate(CLSCTX_ALL, None).ok();
            if let Some(ref sc) = silent_render_client {
                let _ = sc.Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    0,
                    buffer_duration,
                    0,
                    mix_format_ptr,
                    None,
                );
                let _ = sc.Start();
            }

            audio_client.Start()?;

            let mut resampler = AudioResampler::new(sample_rate, 48000, channels);

            while is_running.load(Ordering::Relaxed) {
                while let Ok(packet_size) = capture_client.GetNextPacketSize() {
                    if packet_size == 0 {
                        break;
                    }
                    let mut data_ptr: *mut u8 = std::ptr::null_mut();
                    let mut num_frames = 0u32;
                    let mut flags = 0u32;
                    let mut dev_pos = 0u64;
                    let mut qpc_pos = 0u64;

                    if capture_client
                        .GetBuffer(
                            &mut data_ptr,
                            &mut num_frames,
                            &mut flags,
                            Some(&mut dev_pos),
                            Some(&mut qpc_pos),
                        )
                        .is_ok()
                    {
                        if let Ok(ref clock) = audio_clock {
                            let mut pos = 0u64;
                            let mut qpc = 0u64;
                            if clock.GetPosition(&mut pos, Some(&mut qpc)).is_ok() {
                                *clock_pos.lock().unwrap() = pos;
                            }
                        }

                        let total_samples = (num_frames * channels as u32) as usize;
                        let mut raw_samples = Vec::with_capacity(total_samples);

                        // Bit 1 = AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY (1)
                        // Bit 2 = AUDCLNT_BUFFERFLAGS_SILENT (2)
                        if (flags & 2) != 0 || (flags & 1) != 0 || data_ptr.is_null() {
                            // Silence or discontinuity
                            raw_samples.resize(total_samples, 0.0f32);
                        } else {
                            Self::decode_pcm(data_ptr, total_samples, bits_per_sample, &mut raw_samples);
                        }

                        let mut resampled_48k = Vec::with_capacity(raw_samples.len());
                        resampler.process_interleaved(&raw_samples, &mut resampled_48k);

                        let packet = AudioPacket {
                            samples: resampled_48k,
                            sample_rate: 48000,
                            channels,
                            timestamp_100ns: qpc_pos as i64,
                            hardware_audio_clock_ticks: dev_pos,
                        };

                        on_audio(packet);

                        let _ = capture_client.ReleaseBuffer(num_frames);
                    }
                }

                thread::sleep(Duration::from_millis(5));
            }

            if let Some(ref sc) = silent_render_client {
                let _ = sc.Stop();
            }
            let _ = audio_client.Stop();
            Ok(())
        }
    }

    fn decode_pcm(ptr: *const u8, total_samples: usize, bits_per_sample: u16, out: &mut Vec<f32>) {
        unsafe {
            match bits_per_sample {
                32 => {
                    let float_slice = std::slice::from_raw_parts(ptr as *const f32, total_samples);
                    out.extend_from_slice(float_slice);
                }
                16 => {
                    let i16_slice = std::slice::from_raw_parts(ptr as *const i16, total_samples);
                    for &s in i16_slice {
                        out.push(s as f32 / 32768.0);
                    }
                }
                24 => {
                    let byte_slice = std::slice::from_raw_parts(ptr, total_samples * 3);
                    for chunk in byte_slice.chunks_exact(3) {
                        let val = ((chunk[0] as i32) | ((chunk[1] as i32) << 8) | ((chunk[2] as i8 as i32) << 16)) as f32;
                        out.push(val / 8388608.0);
                    }
                }
                _ => {
                    out.resize(total_samples, 0.0f32);
                }
            }
        }
    }

    pub fn get_master_audio_clock(&self) -> (u64, u64) {
        let pos = *self.latest_clock_pos.lock().unwrap();
        let freq = *self.clock_frequency.lock().unwrap();
        (pos, freq)
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for WasapiLoopbackRecorder {
    fn drop(&mut self) {
        self.stop();
    }
}
