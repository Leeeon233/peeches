use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};

use ringbuffer::{AllocRingBuffer, RingBuffer};

pub struct AudioOutput {
    #[cfg(target_os = "macos")]
    inner: macos::MacAudioOutput,
    #[cfg(target_os = "windows")]
    inner: win::WinAudioOutput,
    sender: mpsc::Sender<Vec<f32>>,
    speech_buf: Arc<Mutex<AllocRingBuffer<f32>>>,
}

unsafe impl Send for AudioOutput {}
unsafe impl Sync for AudioOutput {}

impl AudioOutput {
    pub fn new(sender: mpsc::Sender<Vec<f32>>) -> anyhow::Result<Self> {
        let speech_buf = Arc::new(Mutex::new(AllocRingBuffer::new(16000 * 3)));
        let speech_buf_clone = speech_buf.clone();
        let counter = Arc::new(AtomicUsize::new(0));
        let sender_clone = sender.clone();
        let cb = Box::new(move |data| {
            let mut buf = speech_buf_clone.lock().unwrap();
            buf.extend(data);
            counter.fetch_add(1, Ordering::SeqCst);
            if counter.load(Ordering::SeqCst) > (16000.0 / 320.0 * 0.6) as usize
                && buf.len() as f64 > 1.1 * 16000.0
            {
                let samples = buf.to_vec();
                drop(buf);
                sender_clone.send(samples).unwrap();
                counter.store(0, Ordering::SeqCst);
            }
        });
        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                inner: macos::MacAudioOutput::new(),
            })
        }
        #[cfg(target_os = "windows")]
        {
            Ok(Self {
                inner: win::WinAudioOutput::new(cb)?,
                sender,
                speech_buf,
            })
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.inner.is_stopped()
    }

    pub fn start_recording(&self) -> anyhow::Result<()> {
        self.inner.start_recording()
    }

    pub fn stop_recording(&self) {
        self.inner.stop_recording()
    }
}

#[cfg(target_os = "windows")]
mod win {
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use cpal::traits::DeviceTrait;
    use cpal::traits::HostTrait;
    use cpal::traits::StreamTrait;

    use super::audio_resample;
    use super::stereo_to_mono;

    pub struct WinAudioOutput {
        stream: cpal::Stream,
        is_stopped: Arc<AtomicBool>,
    }

    impl WinAudioOutput {
        pub fn new(on_data: Box<dyn Fn(Vec<f32>) + Send>) -> anyhow::Result<Self> {
            let err_fn = move |err| {
                eprintln!("an error occurred on stream: {}", err);
            };
            let host = cpal::default_host();
            let device = host.default_output_device().unwrap();
            let config = device.default_output_config().unwrap();
            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
                    &config.into(),
                    move |data, _: &_| {
                        // TODO: assume 2 channels
                        let mut resampled: Vec<f32> = audio_resample(data, 48000, 16000, 2);
                        resampled = stereo_to_mono(&resampled).unwrap();
                        on_data(resampled);
                    },
                    err_fn,
                    None,
                )?,
                sample_format => {
                    return Err(anyhow::Error::msg(format!(
                        "Unsupported sample format '{sample_format}'"
                    )))
                }
            };
            Ok(WinAudioOutput {
                stream,
                is_stopped: Arc::new(AtomicBool::new(true)),
            })
        }

        pub fn is_stopped(&self) -> bool {
            self.is_stopped.load(Ordering::SeqCst)
        }

        pub fn start_recording(&self) -> anyhow::Result<()> {
            self.stream.play()?;
            self.is_stopped.store(false, Ordering::SeqCst);
            Ok(())
        }

        pub fn stop_recording(&self) {
            self.stream.pause();
            self.is_stopped.store(true, Ordering::SeqCst);
        }
    }
}

pub fn audio_resample(
    data: &[f32],
    sample_rate0: u32,
    sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    use samplerate::{convert, ConverterType};
    convert(
        sample_rate0 as _,
        sample_rate as _,
        channels as _,
        ConverterType::SincBestQuality,
        data,
    )
    .unwrap_or_default()
}

pub fn stereo_to_mono(stereo_data: &[f32]) -> anyhow::Result<Vec<f32>> {
    let mut mono_data = Vec::with_capacity(stereo_data.len() / 2);

    for chunk in stereo_data.chunks_exact(2) {
        let average = (chunk[0] + chunk[1]) / 2.0;
        mono_data.push(average);
    }

    Ok(mono_data)
}

#[cfg(target_os = "macos")]
mod macos {

    use cidre::{
        arc::Retained,
        cm::{self},
        define_obj_type, dispatch, objc,
        sc::{
            self,
            stream::{Output, OutputImpl},
        },
    };
    use futures::executor::block_on;

    struct StreamOutputInner {
        sender: Sender<Option<Retained<cm::SampleBuf>>>,
    }

    impl StreamOutputInner {
        fn handle_audio(&mut self, sample_buf: &mut cm::SampleBuf) {
            let audio_buf_list = sample_buf.unwrap().audio_buf_list::<2>().unwrap();
            let buffer_list = audio_buf_list.list();
            let samples = unsafe {
                let buffer = buffer_list.buffers[0];
                std::slice::from_raw_parts(
                    buffer.data as *const f32,
                    buffer.data_bytes_size as usize / std::mem::size_of::<f32>(),
                )
            };
            match self.sender.send(Some(samples.clone())) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Failed to send sample buffer: {:?}", e);
                }
            }
        }
    }

    define_obj_type!(StreamOutput + OutputImpl, StreamOutputInner, STREAM_OUTPUT);

    impl Output for StreamOutput {}
    #[objc::add_methods]
    impl OutputImpl for StreamOutput {
        extern "C" fn impl_stream_did_output_sample_buf(
            &mut self,
            _cmd: Option<&cidre::objc::Sel>,
            _stream: &sc::Stream,
            sample_buf: &mut cm::SampleBuf,
            kind: sc::OutputType,
        ) {
            match kind {
                sc::OutputType::Screen => {}
                sc::OutputType::Audio => self.inner_mut().handle_audio(sample_buf),
                sc::OutputType::Mic => {}
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct MacAudioOutput {
        output: Arc<Mutex<Retained<StreamOutput>>>,
        pub sender: Sender<Option<Vec<f32>>>,
        stream: Arc<Mutex<Option<Retained<cidre::sc::Stream>>>>,
        pub stop_signal: Arc<AtomicBool>,
    }

    unsafe impl Send for MacAudioOutput {}
    unsafe impl Sync for MacAudioOutput {}

    impl MacAudioOutput {
        pub fn new() -> Self {
            let (tx, _rx) = broadcast::channel(32);
            let inner = StreamOutputInner { sender: tx.clone() };
            let delegate = StreamOutput::with(inner);

            Self {
                output: Arc::new(Mutex::new(delegate)),
                sender: tx,
                stop_signal: Arc::new(AtomicBool::new(true)),
                stream: Arc::new(Mutex::new(None)),
            }
        }

        pub fn is_stopped(&self) -> bool {
            self.stop_signal.load(Ordering::SeqCst)
        }

        pub fn start_recording(&self) -> bool {
            if !self.is_stopped() {
                log::info!("start_recording: already started");
                return false;
            }
            self.stop_signal.store(false, Ordering::SeqCst);
            let output = self.output.clone();
            let content = block_on(sc::ShareableContent::current()).unwrap();
            let displays = content.displays().clone();
            let display = displays.first().expect("No display found");
            let filter = sc::ContentFilter::with_display_excluding_windows(
                display,
                &cidre::ns::Array::new(),
            );

            let queue = dispatch::Queue::serial_with_ar_pool();
            let mut cfg = sc::StreamCfg::new();
            cfg.set_captures_audio(true);
            cfg.set_excludes_current_process_audio(false);

            let stream = sc::Stream::new(&filter, &cfg);
            *self.stream.try_lock().unwrap() = Some(stream.retained());

            stream
                .add_stream_output(
                    output.try_lock().unwrap().as_ref(),
                    sc::OutputType::Audio,
                    Some(&queue),
                )
                .expect("Failed to add stream output");

            block_on(stream.start()).unwrap();
            log::info!("stream started");
            true
        }

        pub fn stop_recording(&self) {
            if self.is_stopped() {
                return;
            }
            self.stop_signal.store(true, Ordering::SeqCst);
            let mut stream = self.stream.try_lock().unwrap();
            if let Some(stream) = stream.as_mut() {
                block_on(stream.stop()).expect("Failed to stop stream");
            }
            let _ = self.sender.send(None);
        }
    }
}
