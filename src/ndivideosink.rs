use glib;
use glib::subclass;
use glib::subclass::prelude::*;
use gst;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base;
use gst_base::subclass::prelude::*;
use std::sync::Mutex;

use crate::ndisys::*;
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

enum State {
    Stopped,
    Started { sender: SendInstance },
}

impl Default for State {
    fn default() -> State {
        State::Stopped
    }
}

pub(crate) struct NdiVideoSink {
    cat: gst::DebugCategory,
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

impl ObjectSubclass for NdiVideoSink {
    const NAME: &'static str = "RsNDISink";
    type ParentType = gst_base::BaseSink;
    type Instance = gst::subclass::ElementInstanceStruct<Self>;
    type Class = subclass::simple::ClassStruct<Self>;

    glib_object_subclass!();

    fn new() -> Self {
        Self {
            cat: gst::DebugCategory::new(
                "ndivideosink",
                gst::DebugColorFlags::empty(),
                Some("NewTek NDI Video Sink"),
            ),
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
        }
    }

    fn class_init(klass: &mut subclass::simple::ClassStruct<Self>) {
        klass.set_metadata(
            "NewTek NDI Video Sink",
            "Sink",
            "NewTek NDI video Sink",
            "Ruben Gonzalez <rubenrua@teltek.es>, Daniel Vilar <daniel.peiteado@teltek.es>, Sebastian Dröge <sebastian@centricular.com>, Luke Moscrop <luke.moscrop@bbc.co.uk>",
        );

        let caps = gst::Caps::new_any();
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

impl ObjectImpl for NdiVideoSink {
    glib_object_impl!();

    fn set_property(&self, obj: &glib::Object, id: usize, value: &glib::Value) {
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

impl ElementImpl for NdiVideoSink {}

impl BaseSinkImpl for NdiVideoSink {
    fn start(&self, _element: &gst_base::BaseSink) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();

        if let State::Started { .. } = *state {
            unreachable!("ndivideosink already started");
        }

        let sender = create_send_instance(settings.ndi_name.clone(), false, false);
        *state = State::Started{ sender: sender.unwrap() };

        Ok(())
    }

    fn stop(&self, element: &gst_base::BaseSink) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        *state = State::Stopped;
        gst_info!(self.cat, obj: element, "Stopped");

        Ok(())
    }

    fn render(&self, element: &gst_base::BaseSink, buffer: &gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut state = self.state.lock().unwrap();
        let (sender) = match *state {
            State::Started {
                ref mut sender,
            } => (sender),
            State::Stopped => {
                gst_element_error!(element, gst::CoreError::Failed, ["Not started yet"]);
                return Err(gst::FlowError::Error);
            }
        };

        gst_trace!(self.cat, obj: element, "Rendering {:?}", buffer);
        let map = buffer.map_readable().ok_or_else(|| {
            gst_element_error!(element, gst::CoreError::Failed, ["Failed to map buffer"]);
            gst::FlowError::Error
        })?;

        let frame = create_ndi_send_video_frame(
            1280,
            720,
            NDIlib_frame_format_type_e::NDIlib_frame_format_type_progressive,
        )
        .with_data(map.as_ref().to_vec(), 1280 * 4)
        .build();

        sender.send_video(frame.unwrap());

        Ok(gst::FlowSuccess::Ok)
    }
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "ndivideosink",
        gst::Rank::None,
        NdiVideoSink::get_type(),
    )
}
