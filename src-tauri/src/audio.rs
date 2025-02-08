use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tokio::sync::{
    broadcast::{self, Sender},
    Mutex,
};

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
        match self.sender.send(Some(sample_buf.retained())) {
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
pub struct AudioOutput {
    output: Arc<Mutex<Retained<StreamOutput>>>,
    pub sender: Sender<Option<Retained<cm::SampleBuf>>>,
    stream: Arc<Mutex<Option<Retained<cidre::sc::Stream>>>>,
    pub stop_signal: Arc<AtomicBool>,
}

unsafe impl Send for AudioOutput {}
unsafe impl Sync for AudioOutput {}

impl AudioOutput {
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
        let filter =
            sc::ContentFilter::with_display_excluding_windows(display, &cidre::ns::Array::new());

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
