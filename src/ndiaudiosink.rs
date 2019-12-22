use glib;
use glib::subclass;
use glib::subclass::prelude::*;
use gst;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base;
use gst_base::subclass::prelude::*;
use std::sync::Mutex;
use std::i32;

use crate::send::*;

use crate::DEFAULT_RECEIVER_NDI_NAME;

#[derive(Debug)]
struct Settings {
    ndi_name: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ndi_name: DEFAULT_RECEIVER_NDI_NAME.clone(),
        }
    }
}

static PROPERTIES: [subclass::Property; 1] = [
    subclass::Property("ndi-name", |name| {
        glib::ParamSpec::string(
            name,
            "NDI Name",
            "The name of the NDI stream",
            None,
            glib::ParamFlags::READWRITE,
        )
    })
];

struct State {
    audio_info: Option<gst_audio::AudioInfo>,
    sender: Option<SendInstance>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sender: None,
            audio_info: None
        }
    }
}

pub(crate) struct NdiAudioSink {
    cat: gst::DebugCategory,
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

impl ObjectSubclass for NdiAudioSink {
    const NAME: &'static str = "RsNDIAudioSink";
    type ParentType = gst_base::BaseSink;
    type Instance = gst::subclass::ElementInstanceStruct<Self>;
    type Class = subclass::simple::ClassStruct<Self>;

    glib_object_subclass!();

    fn new() -> Self {
        Self {
            cat: gst::DebugCategory::new(
                "ndiaudiosink",
                gst::DebugColorFlags::empty(),
                Some("NewTek NDI Audio Sink"),
            ),
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
        }
    }

    fn class_init(klass: &mut subclass::simple::ClassStruct<Self>) {
        klass.set_metadata(
            "NewTek NDI Audio Sink",
            "Sink",
            "NewTek NDI Audio Sink",
            "Ruben Gonzalez <rubenrua@teltek.es>, Daniel Vilar <daniel.peiteado@teltek.es>, Sebastian Dröge <sebastian@centricular.com>, Luke Moscrop <luke.moscrop@bbc.co.uk>",
        );

        let caps = gst::Caps::new_simple(
            "audio/x-raw",
            &[
                (
                    "format",
                    &gst::List::new(&[
                        &gst_audio::AudioFormat::S16le.to_string(),
                    ]),
                ),
                ("rate", &gst::IntRange::<i32>::new(0, i32::MAX)),
                ("channels", &gst::IntRange::<i32>::new(0, i32::MAX)),
            ],
        );

        let sink_pad_template = gst::PadTemplate::new(
            "sink",
            gst::PadDirection::Sink,
            gst::PadPresence::Always,
            &caps,
        )
        .unwrap();

        klass.add_pad_template(sink_pad_template);
        klass.install_properties(&PROPERTIES);
    }
}

impl ObjectImpl for NdiAudioSink {
    glib_object_impl!();

    fn set_property(&self, _obj: &glib::Object, id: usize, value: &glib::Value) {
        let prop = &PROPERTIES[id];
        match *prop {
            subclass::Property("ndi-name", ..) => {
                let mut settings = self.settings.lock().unwrap();
                let ndi_name = value
                    .get()
                    .unwrap_or_else(|| DEFAULT_RECEIVER_NDI_NAME.clone());
                
                settings.ndi_name = ndi_name;
            }
            _ => unimplemented!(),
        }
    }

    fn get_property(&self, _obj: &glib::Object, id: usize) -> Result<glib::Value, ()> {
        let prop = &PROPERTIES[id];

        match *prop {
            subclass::Property("ndi-name", ..) => {
                let settings = self.settings.lock().unwrap();
                Ok(settings.ndi_name.to_value())
            }
            _ => unimplemented!(),
        }
    }
}

impl ElementImpl for NdiAudioSink {}

impl BaseSinkImpl for NdiAudioSink {
    fn start(&self, _element: &gst_base::BaseSink) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();


        let sender = create_send_instance(settings.ndi_name.clone(), false, false);
        state.sender = Some(sender.unwrap());

        Ok(())
    }

    fn set_caps(&self, _element: &gst_base::BaseSink, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let mut state = self.state.lock().unwrap();
        state.audio_info = gst_audio::AudioInfo::from_caps(caps);
        Ok(())
    }

    fn render(&self, element: &gst_base::BaseSink, buffer: &gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();

        gst_trace!(self.cat, obj: element, "Rendering {:?}", buffer);
        let map = buffer.map_readable().ok_or_else(|| {
            gst_element_error!(element, gst::CoreError::Failed, ["Failed to map buffer"]);
            gst::FlowError::Error
        })?;

        if let Some(ref audio_info) = state.audio_info {
            
            let frame = create_ndi_send_audio_frame(
                audio_info.rate() as i32,
                audio_info.channels() as i32
            )
            .with_data(map.as_ref().to_vec())
            .build();


            if let Some(ref mut sender) = state.sender {
                sender.send_audio(frame.unwrap());
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ndiaudiosink",
        gst::Rank::None,
        NdiAudioSink::get_type(),
    )
}
