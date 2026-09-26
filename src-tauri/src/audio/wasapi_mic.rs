use crate::audio::resampler::AudioResampler;
use crate::audio::wasapi_loopback::AudioPacket;
use crate::error::{PanGlideError, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, IAudioCaptureClient, IAudioClient,
    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_HIGHEST};

pub type VuCallback = Arc<dyn Fn(f32, f32) + Send + Sync + 'static>;
pub type MicCallback = Arc<dyn Fn(AudioPacket) + Send + Sync + 'static>;

pub struct WasapiMicRecorder {
    is_running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    latest_vu: Arc<Mutex<(f32, f32)>>,
}

impl WasapiMicRecorder {
    pub fn start(on_mic_audio: MicCallback, on_vu: Option<VuCallback>) -> Result<Self> {
        let is_running = Arc::new(AtomicBool::new(true));
        let running_flag = is_running.clone();
        let latest_vu = Arc::new(Mutex::new((0.0f32, 0.0f32)));
        let vu_clone = latest_vu.clone();

        let thread_handle = thread::Builder::new()
            .name("panglide-wasapi-mic".into())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                    let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
                }

                let run_result = Self::run_loop(running_flag, vu_clone, on_mic_audio, on_vu);
                if let Err(e) = run_result {
                    eprintln!("[WasapiMicRecorder] error: {:?}", e);
                }

                unsafe {
                    CoUninitialize();
                }
            })
            .map_err(|e| PanGlideError::Audio(format!("Failed to spawn mic thread: {}", e)))?;

        Ok(Self {
            is_running,
            thread_handle: Some(thread_handle),
            latest_vu,
        })
    }

    fn run_loop(
        is_running: Arc<AtomicBool>,
        latest_vu: Arc<Mutex<(f32, f32)>>,
        on_audio: MicCallback,
        on_vu: Option<VuCallback>,
    ) -> Result<()> {
        unsafe {
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)?;
            let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

            let mix_format_ptr = audio_client.GetMixFormat()?;
            let mix_format = *mix_format_ptr;
            let sample_rate = mix_format.nSamplesPerSec;
            let channels = mix_format.nChannels;
            let bits_per_sample = mix_format.wBitsPerSample;

            let buffer_duration: i64 = 1_000_000; // 100ms
            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                0,
                buffer_duration,
                0,
                mix_format_ptr,
                None,
            )?;

            let capture_client: IAudioCaptureClient = audio_client.GetService()?;
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
                        let total_samples = (num_frames * channels as u32) as usize;
                        let mut raw_samples = Vec::with_capacity(total_samples);

                        if (flags & 1) != 0 || data_ptr.is_null() {
                            raw_samples.resize(total_samples, 0.0f32);
                        } else {
                            match bits_per_sample {
                                32 => {
                                    let float_slice = std::slice::from_raw_parts(data_ptr as *const f32, total_samples);
                                    raw_samples.extend_from_slice(float_slice);
                                }
                                16 => {
                                    let i16_slice = std::slice::from_raw_parts(data_ptr as *const i16, total_samples);
                                    for &s in i16_slice {
                                        raw_samples.push(s as f32 / 32768.0);
                                    }
                                }
                                _ => {
                                    raw_samples.resize(total_samples, 0.0f32);
                                }
                            }
                        }

                        // Calculate live VU meter
                        let (rms, peak) = AudioResampler::calculate_vu_meter(&raw_samples);
                        *latest_vu.lock().unwrap() = (rms, peak);
                        if let Some(ref cb) = on_vu {
                            cb(rms, peak);
                        }

                        // Resample to 48 kHz
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

            let _ = audio_client.Stop();
            Ok(())
        }
    }

    pub fn get_vu(&self) -> (f32, f32) {
        *self.latest_vu.lock().unwrap()
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for WasapiMicRecorder {
    fn drop(&mut self) {
        self.stop();
    }
}
