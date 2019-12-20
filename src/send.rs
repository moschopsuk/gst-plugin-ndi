use std::ffi::CString;
use std::ptr::{null, null_mut};
use std::os::raw::c_char;

use super::*;

unsafe impl Send for SendInstance {}

pub struct SendInstance {
    instance: NDIlib_send_instance_t,
}

impl Drop for SendInstance {
    fn drop(&mut self) {
        unsafe {
            NDIlib_send_destroy(self.instance);
        }
    }
}

impl SendInstance {
    pub fn send_video(&mut self, frame: NDISendVideoFrame) {
        unsafe {
            NDIlib_send_send_video_v2(self.instance, &frame.instance);
        }
    }
}

pub struct NDISendVideoFrameBuilder {
    instance: NDIlib_video_frame_v2_t,
    metadata: Option<String>,
    data: Vec<u8>,
}

impl NDISendVideoFrameBuilder {
    pub fn with_data(mut self, data: Vec<u8>, line_stride: i32) -> Self {
        self.data = data;
        self.instance.line_stride_in_bytes = line_stride;
        self
    }

    pub fn with_format(mut self, video_format: gst_video::VideoFormat) -> Self {
        //TODO: fix unreachable_patterns warning
        let format = match video_format {
            gst_video::VideoFormat::Uyvy => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_UYVY,
            gst_video::VideoFormat::I420 => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_YV12,
            gst_video::VideoFormat::Nv12 => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_NV12,
            gst_video::VideoFormat::Yv12 => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_I420,
            gst_video::VideoFormat::Bgra => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_BGRA,
            gst_video::VideoFormat::Bgrx => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_BGRX,
            gst_video::VideoFormat::Rgba => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_RGBA,
            gst_video::VideoFormat::Rgbx => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_RGBX,
            gst_video::VideoFormat::Uyvy => ndisys::NDIlib_FourCC_type_e::NDIlib_FourCC_type_UYVA,
            _ => panic!("Unkown format"),
        };

        self.instance.FourCC = format;
        self
    }

    pub fn build(self) -> Result<NDISendVideoFrame, SendCreateError> {
        let mut res = NDISendVideoFrame {
            instance: self.instance,
            metadata: self.metadata,
            data: self.data,
        };

        res.data.resize((res.instance.line_stride_in_bytes * res.instance.yres) as usize, 0);
        res.instance.p_data = res.data.as_mut_ptr() as *const c_char;

        Ok(res)
    }
}

pub fn create_ndi_send_video_frame(width: i32, height: i32, frame_type: NDIlib_frame_format_type_e) -> NDISendVideoFrameBuilder {
    NDISendVideoFrameBuilder {
        instance: NDIlib_video_frame_v2_t {
            xres: width,
            yres: height,
            FourCC: NDIlib_FourCC_type_e::NDIlib_FourCC_type_RGBA,
            frame_rate_N: 0,
            frame_rate_D: 0,
            picture_aspect_ratio: 0.0,
            frame_format_type: frame_type,
            timecode: NDIlib_send_timecode_synthesize,
            p_data: null_mut(),
            line_stride_in_bytes: 0,
            p_metadata: null(),
            timestamp: 0,
        },
        metadata: None,
        data: vec![],
    }
}

#[derive(Debug)]
pub struct NDISendVideoFrame {
    instance: NDIlib_video_frame_v2_t,
    metadata: Option<String>,
    data: Vec<u8>,
}

#[derive(Debug)]
pub enum SendCreateError {
    InvalidName,
    Failed,
}

pub fn create_send_instance(
    name: String,
    clock_video: bool,
    clock_audio: bool,
) -> Result<SendInstance, SendCreateError> {
    let name2 = CString::new(name.as_bytes()).map_err(|_| SendCreateError::InvalidName)?;

    let props = NDIlib_send_create_t {
        p_ndi_name: name2.as_ptr(),
        p_groups: null(),
        clock_video,
        clock_audio,
    };

    let instance = unsafe { NDIlib_send_create(&props) };

    if instance.is_null() {
        Err(SendCreateError::Failed)
    } else {
        Ok(SendInstance {
            instance,
        })
    }
}