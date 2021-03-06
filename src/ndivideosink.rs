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

struct State {
    video_info: Option<gst_video::VideoInfo>,
    sender: Option<SendInstance>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sender: None,
            video_info: None
        }
    }
}

pub(crate) struct NdiVideoSink {
    cat: gst::DebugCategory,
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

impl ObjectSubclass for NdiVideoSink {
    const NAME: &'static str = "RsNDIVideoSink";
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

        let caps = gst::Caps::new_simple(
            "video/x-raw",
            &[
                (
                    "format",
                    &gst::List::new(&[
                        &gst_video::VideoFormat::Uyvy.to_string(),
                        &gst_video::VideoFormat::Yv12.to_string(),
                        &gst_video::VideoFormat::Nv12.to_string(),
                        &gst_video::VideoFormat::I420.to_string(),
                        &gst_video::VideoFormat::Bgra.to_string(),
                        &gst_video::VideoFormat::Bgrx.to_string(),
                        &gst_video::VideoFormat::Rgba.to_string(),
                        &gst_video::VideoFormat::Rgbx.to_string(),
                    ]),
                ),
                ("width", &gst::IntRange::<i32>::new(0, i32::MAX)),
                ("height", &gst::IntRange::<i32>::new(0, i32::MAX)),
                (
                    "framerate",
                    &gst::FractionRange::new(
                        gst::Fraction::new(0, 1),
                        gst::Fraction::new(i32::MAX, 1),
                    ),
                ),
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

impl ObjectImpl for NdiVideoSink {
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

impl ElementImpl for NdiVideoSink {}

impl BaseSinkImpl for NdiVideoSink {
    fn start(&self, _element: &gst_base::BaseSink) -> Result<(), gst::ErrorMessage> {
        let mut state = self.state.lock().unwrap();
        let settings = self.settings.lock().unwrap();


        let sender = create_send_instance(settings.ndi_name.clone(), false, false);
        state.sender = Some(sender.unwrap());

        Ok(())
    }

    fn set_caps(&self, _element: &gst_base::BaseSink, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let mut state = self.state.lock().unwrap();
        state.video_info = gst_video::VideoInfo::from_caps(caps);
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

        if let Some(ref video_info) = state.video_info {
            //TODO: find better way to get stride out
            let in_frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer.as_ref(), &video_info).unwrap();

            let frame = create_ndi_send_video_frame(
                video_info.width() as i32,
                video_info.height() as i32,
                NDIlib_frame_format_type_e::NDIlib_frame_format_type_progressive,
            )
            .with_format(video_info.format())
            .with_data(map.as_ref().to_vec(), in_frame.plane_stride()[0] as i32)
            .build();

            if let Some(ref mut sender) = state.sender {
                sender.send_video(frame.unwrap());
            }
        }

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
