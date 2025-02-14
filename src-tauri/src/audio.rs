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
}

unsafe impl Send for AudioOutput {}
unsafe impl Sync for AudioOutput {}

impl AudioOutput {
    pub fn new(sender: mpsc::Sender<Vec<f32>>) -> anyhow::Result<Self> {
        let speech_buf = Arc::new(Mutex::new(AllocRingBuffer::new(16000 * 3)));
        let counter = Arc::new(AtomicUsize::new(0));
        let cb = Box::new(move |data| {
            let mut buf = speech_buf.lock().unwrap();
            buf.extend(data);
            counter.fetch_add(1, Ordering::SeqCst);
            if counter.load(Ordering::SeqCst) > (16000.0 / 320.0 * 0.6) as usize
                && buf.len() as f64 > 1.1 * 16000.0
            {
                let samples = buf.to_vec();
                drop(buf);
                sender.send(samples).unwrap();
                counter.store(0, Ordering::SeqCst);
            }
        });
        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                inner: macos::MacAudioOutput::new(cb),
            })
        }
        #[cfg(target_os = "windows")]
        {
            Ok(Self {
                inner: win::WinAudioOutput::new(cb)?,
            })
        }
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
            Ok(WinAudioOutput { stream })
        }

        pub fn start_recording(&self) -> anyhow::Result<()> {
            self.stream.play()?;

            Ok(())
        }

        pub fn stop_recording(&self) {
            self.stream.pause();
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

    use super::audio_resample;

    struct StreamOutputInner {
        on_data: Box<dyn Fn(Vec<f32>) + Send>,
    }

    impl StreamOutputInner {
        fn handle_audio(&mut self, sample_buf: &mut cm::SampleBuf) {
            let audio_buf_list = sample_buf.audio_buf_list::<2>().unwrap();
            let buffer_list = audio_buf_list.list();
            let samples = unsafe {
                let buffer = buffer_list.buffers[0];
                std::slice::from_raw_parts(
                    buffer.data as *const f32,
                    buffer.data_bytes_size as usize / std::mem::size_of::<f32>(),
                )
            };
            let resampled: Vec<f32> = audio_resample(samples, 48000, 16000, 1);
            (self.on_data)(resampled);
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

    pub struct MacAudioOutput {
        output: Retained<StreamOutput>,
        stream: Retained<cidre::sc::Stream>,
    }

    unsafe impl Send for MacAudioOutput {}
    unsafe impl Sync for MacAudioOutput {}

    impl MacAudioOutput {
        pub fn new(on_data: Box<dyn Fn(Vec<f32>) + Send>) -> Self {
            let inner = StreamOutputInner { on_data };
            let delegate = StreamOutput::with(inner);
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
            stream
                .add_stream_output(delegate.as_ref(), sc::OutputType::Audio, Some(&queue))
                .expect("Failed to add stream output");
            Self {
                output: delegate,
                stream,
            }
        }

        pub fn start_recording(&self) -> anyhow::Result<()> {
            block_on(self.stream.start())?;
            log::info!("stream started");
            Ok(())
        }

        pub fn stop_recording(&self) {
            block_on(self.stream.stop()).unwrap();
        }
    }
}
